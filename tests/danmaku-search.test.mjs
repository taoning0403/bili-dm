import test from "node:test";
import assert from "node:assert/strict";
import { searchTitle } from "../src/frontend/lib/danmakuSearch.ts";
test("search title preserves season, episode and year while removing release metadata", () => {
  assert.equal(searchTitle("/video/[Group] Show.S02E03.2024.1080p.WEB-DL.x265.AAC.mkv"), "Show 第2季 第3集 2024");
  assert.equal(searchTitle("作品 第12集 1080p.mkv"), "作品 第12集");
  assert.equal(searchTitle("C:\\Videos\\Title.E07.mp4"), "Title 第7集");
});
