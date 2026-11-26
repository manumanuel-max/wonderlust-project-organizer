import { describe, expect, it } from "vitest";
import { basename, formatDuration, formatSize, joinPath, percent } from "./format";

describe("format", () => {
  it("formatea tamaños en español", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1536)).toBe("1,50 KB");
    expect(formatSize(150 * 1024 ** 3)).toBe("150 GB");
    expect(formatSize(null)).toBe("—");
  });

  it("formatea duraciones", () => {
    expect(formatDuration(5000)).toBe("5 s");
    expect(formatDuration(65_000)).toBe("1 min 05 s");
    expect(formatDuration(3_720_000)).toBe("1 h 02 min");
  });

  it("une rutas con el separador correcto", () => {
    expect(joinPath("/Volumes/SSD", "P")).toBe("/Volumes/SSD/P");
    expect(joinPath("D:\\Backups", "P")).toBe("D:\\Backups\\P");
    expect(joinPath("D:\\", "P")).toBe("D:\\P");
    expect(joinPath("/", "P")).toBe("/P");
  });

  it("obtiene el nombre base", () => {
    expect(basename("/a/b/Campaña Navidad")).toBe("Campaña Navidad");
    expect(basename("C:\\x\\y.mov")).toBe("y.mov");
  });

  it("calcula porcentajes acotados", () => {
    expect(percent(5, 10)).toBe(50);
    expect(percent(5, 0)).toBe(0);
    expect(percent(20, 10)).toBe(100);
  });
});
