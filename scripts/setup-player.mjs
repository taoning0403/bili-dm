import { createHash } from "node:crypto";
import { mkdtemp, readFile, writeFile, mkdir, cp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

// Pinned native runtime, built by Frame Player from LGPL mpv/FFmpeg.
// Source/build/patch provenance is recorded in THIRD-PARTY-NOTICES.md.
const name = "macos-arm64-9cb06b14559c";
const sha256 = "4d5b1eb5ea390d1dadd388fba88aa9a7ca41177188ba367b03ab67d75db5b796";
if (process.platform !== "darwin" || process.arch !== "arm64") {
  throw new Error("此自动安装脚本仅提供 macOS Apple Silicon 运行库。其他平台请按 README 配置 libmpv。");
}
const output = fileURLToPath(new URL("../src-tauri/lib/", import.meta.url));
const stamp = join(output, ".runtime-sha256");
if (await readFile(stamp, "utf8").catch(() => "") === sha256) {
  await readFile(join(output, "libmpv.dylib"));
  console.log("播放器运行库已就绪。");
  process.exit(0);
}
const work = await mkdtemp(join(tmpdir(), "bili-player-"));
try {
  const archive = join(work, "runtime.tar.gz");
  execFileSync("curl", ["-fL", "--retry", "2", "--max-time", "180", `https://libs.frameplayer.app/${name}.tar.gz`, "-o", archive], { stdio: "inherit" });
  const digest = createHash("sha256").update(await readFile(archive)).digest("hex");
  if (digest !== sha256) throw new Error("运行库 SHA-256 校验失败，未安装。");
  const names = execFileSync("tar", ["-tzf", archive], { encoding: "utf8" }).split("\n").filter(Boolean);
  if (names.some(p => p.startsWith("/") || p.split("/").includes(".."))) throw new Error("归档路径不安全。");
  execFileSync("tar", ["-xzf", archive, "-C", work, "lib"]);
  await mkdir(output, { recursive: true });
  await cp(join(work, "lib"), output, { recursive: true, dereference: false });
  await rm(join(output, "libmpv-wrapper.dylib"), { force: true });
  await writeFile(stamp, sha256);
  console.log("已安装带 macOS 嵌入支持的 libmpv 运行库。");
} finally {
  await rm(work, { recursive: true, force: true });
}
