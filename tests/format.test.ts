import { describe, expect, it } from "vitest";
import { formatBytes, formatDuration, formatPercent, formatUptime, isDangerousCommand, shortHash, timeAgo } from "../src/lib/format";

describe("format", () => {
  it("formats bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 ** 3)).toBe("5.0 GB");
    expect(formatBytes(null)).toBe("—");
  });
  it("formats uptime, percent, duration", () => {
    expect(formatUptime(90061)).toBe("1 hari 1 jam");
    expect(formatUptime(300)).toBe("5 menit");
    expect(formatPercent(42.456, 1)).toBe("42.5%");
    expect(formatPercent(null)).toBe("—");
    expect(formatDuration(250)).toBe("250 ms");
    expect(formatDuration(2500)).toBe("2.5 dtk");
  });
  it("formats relative time and hashes", () => {
    expect(timeAgo(1000, 1000)).toBe("baru saja");
    expect(timeAgo(0, 30_000)).toBe("30 dtk lalu");
    expect(shortHash("abcdef123456")).toBe("abcdef12");
    expect(shortHash(null)).toBe("—");
  });
});

describe("dangerous command detection", () => {
  it.each(["rm -rf /var/www", "sudo mkfs.ext4 /dev/sdb", "dd if=/dev/zero of=/dev/sda", "reboot", "mysql -e 'DROP DATABASE x'", "git reset --hard", "chmod -R 777 /"])(
    "flags %s", (c) => expect(isDangerousCommand(c)).toBe(true),
  );
  it.each(["ls -la", "systemctl status nginx", "cat /var/log/syslog", "rm file.txt", "git status"])(
    "allows %s", (c) => expect(isDangerousCommand(c)).toBe(false),
  );
});
