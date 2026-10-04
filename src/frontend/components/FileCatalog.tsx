import { useMemo } from "react";
import { formatBytes } from "../lib/format";
import type { TorrentCatalog, TorrentFile } from "../types/torrent";

interface Directory { name: string; directories: Map<string, Directory>; files: TorrentFile[] }
interface Props { catalog: TorrentCatalog; selected: number | null; onSelect: (index: number) => void }

function buildTree(files: TorrentFile[]): Directory {
  const root: Directory = { name: "", directories: new Map(), files: [] };
  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path, undefined, { numeric: true }));
  for (const file of sorted) {
    const components = file.path.split("/");
    let parent = root;
    for (const name of components.slice(0, -1)) {
      let next = parent.directories.get(name);
      if (!next) { next = { name, directories: new Map(), files: [] }; parent.directories.set(name, next); }
      parent = next;
    }
    parent.files.push(file);
  }
  return root;
}

function FileRows({ directory, selected, onSelect }: { directory: Directory } & Omit<Props, "catalog">) {
  return <ul className="file-tree">
    {[...directory.directories.values()].map((child) => <li key={`dir-${child.name}`}>
      <details open><summary>{child.name}/</summary><FileRows directory={child} selected={selected} onSelect={onSelect} /></details>
    </li>)}
    {directory.files.map((file) => <li key={file.index}>
      <label className={`file-row ${selected === file.index ? "selected" : ""}`}>
        <input type="radio" name="video-file" aria-label={file.path}
          disabled={file.kind !== "video" || file.size === 0} checked={selected === file.index}
          onChange={() => onSelect(file.index)} />
        <span className="file-path" title={file.path}>{file.path.split("/").at(-1)}</span>
        <span className={`file-kind ${file.kind}`}>{({ video: "视频", subtitle: "字幕", other: "其他" })[file.kind]}</span>
        <span className="file-size">{formatBytes(file.size)}</span>
      </label>
    </li>)}
  </ul>;
}

export function FileCatalog({ catalog, selected, onSelect }: Props) {
  const tree = useMemo(() => buildTree(catalog.files), [catalog]);
  const videos = catalog.files.filter((file) => file.kind === "video" && file.size > 0).length;
  const subtitles = catalog.files.filter((file) => file.kind === "subtitle").length;
  return <section className="file-panel" aria-label="磁力文件目录">
    <h2>{catalog.name}</h2>
    <p className="muted">{catalog.files.length} 个文件 · {videos} 个视频 · {subtitles} 个字幕</p>
    {!videos && <p className="error-message">没有识别到支持的视频文件。</p>}
    <FileRows directory={tree} selected={selected} onSelect={onSelect} />
  </section>;
}
