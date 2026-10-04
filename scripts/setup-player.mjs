import { createHash } from "node:crypto";
import { mkdtemp, readFile, writeFile, mkdir, cp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

// Pinned LGPL builds. Source/build provenance is in THIRD-PARTY-NOTICES.md.
const runtimes = {
  "darwin-arm64": {
    url: "https://libs.frameplayer.app/macos-arm64-9cb06b14559c.tar.gz",
    sha256: "4d5b1eb5ea390d1dadd388fba88aa9a7ca41177188ba367b03ab67d75db5b796",
    library: "libmpv.dylib",
    entry: "lib",
  },
  "win32-x64": {
    url: "https://github.com/zhongfly/mpv-winbuild/releases/download/2026-10-03-413ff0b1cd/mpv-dev-lgpl-x86_64-20261003-git-413ff0b1cd.7z",
    sha256: "12a9966bad239672c97276f01a9e625504e0b1d1fdcff95256066eeb8ffb1f21",
    library: "libmpv-2.dll",
    entry: "libmpv-2.dll",
  },
};
const runtime = runtimes[`${process.platform}-${process.arch}`];
if (!runtime) throw new Error("自动安装支持 macOS Apple Silicon 和 Windows x64。其他平台请按 README 配置 libmpv。");
const { sha256 } = runtime;
// Windows' bundled bsdtar supports 7z; Git/MSYS tar may not.
const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
const curl = process.platform === "win32" ? "curl.exe" : "curl";
const output = fileURLToPath(new URL("../src-tauri/lib/", import.meta.url));
const stamp = join(output, ".runtime-sha256");
if (await readFile(stamp, "utf8").catch(() => "") === sha256) {
  await readFile(join(output, runtime.library));
  console.log("播放器运行库已就绪。");
  process.exit(0);
}
const work = await mkdtemp(join(tmpdir(), "bili-player-"));
try {
  const archive = join(work, "runtime.archive");
  execFileSync(curl, ["-fL", "--retry", "2", "--max-time", "180", runtime.url, "-o", archive], { stdio: "inherit" });
  const digest = createHash("sha256").update(await readFile(archive)).digest("hex");
  if (digest !== sha256) throw new Error("运行库 SHA-256 校验失败，未安装。");
  const names = execFileSync(tar, ["-tf", archive], { encoding: "utf8" }).split(/\r?\n/).filter(Boolean);
  if (names.some(p => /^[\\/]|^[a-z]:/i.test(p) || p.split(/[\\/]/).includes(".."))) throw new Error("归档路径不安全。");
  execFileSync(tar, ["-xf", archive, "-C", work, runtime.entry]);
  await mkdir(output, { recursive: true });
  if (process.platform === "win32") {
    await cp(join(work, runtime.library), join(output, runtime.library));
  } else {
    await cp(join(work, "lib"), output, { recursive: true, dereference: false });
    await rm(join(output, "libmpv-wrapper.dylib"), { force: true });
  }
  await writeFile(stamp, sha256);
  console.log(`已安装 ${process.platform}-${process.arch} libmpv 运行库。`);
} finally {
  await rm(work, { recursive: true, force: true });
}
