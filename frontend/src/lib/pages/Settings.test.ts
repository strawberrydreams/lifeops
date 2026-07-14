import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  desktop: false,
  getSystemInfo: vi.fn(),
  getAutostart: vi.fn(),
  setAutostart: vi.fn(),
  openDataDir: vi.fn(),
  importFromDir: vi.fn(),
  getConfig: vi.fn(),
  putConfig: vi.fn(),
  createBackup: vi.fn(),
  listBackups: vi.fn(),
  restoreSnapshot: vi.fn(),
  relaunchApp: vi.fn(),
}));

vi.mock("../api", () => ({
  getSystemInfo: mocks.getSystemInfo,
  getConfig: mocks.getConfig,
  putConfig: mocks.putConfig,
  createBackup: mocks.createBackup,
  listBackups: mocks.listBackups,
}));
vi.mock("../tauri", () => ({
  isDesktop: () => mocks.desktop,
  getAutostart: mocks.getAutostart,
  setAutostart: mocks.setAutostart,
  openDataDir: mocks.openDataDir,
  importFromDir: mocks.importFromDir,
  restoreSnapshot: mocks.restoreSnapshot,
  relaunchApp: mocks.relaunchApp,
}));

import Settings from "./Settings.svelte";

const info = {
  data_dir: "/Users/me/Library/Application Support/LifeOps",
  port: 3000,
  lan_addrs: ["http://192.168.0.2:3000"],
  bind_scope: "localhost" as const,
};

beforeEach(() => {
  mocks.getConfig.mockResolvedValue({ bind_scope: "localhost", backup_dir: null, backup_keep: 7 });
  mocks.listBackups.mockResolvedValue({ backup_dir: "/b", accessible: true, last_success: null, snapshots: [] });
});

afterEach(() => {
  cleanup();
  mocks.desktop = false;
  vi.clearAllMocks();
});

describe("Settings", () => {
  it("데이터 경로·LAN 주소를 렌더하고 브라우저에서는 desktop 기능을 완전히 숨긴다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);

    render(Settings);

    expect(await screen.findByText(info.lan_addrs[0])).toBeInTheDocument();
    expect(screen.getByText(info.data_dir)).toBeInTheDocument();
    expect(screen.queryByText("로그인 시 자동 시작")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "폴더 열기" })).not.toBeInTheDocument();
    expect(screen.queryByText("데이터 가져오기")).not.toBeInTheDocument();
    expect(mocks.getAutostart).not.toHaveBeenCalled();
    expect(mocks.openDataDir).not.toHaveBeenCalled();
  });

  it("데스크탑 가져오기는 경로를 전달하고 재시작 적용을 안내한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.importFromDir.mockResolvedValue(undefined);

    render(Settings);
    const input = await screen.findByLabelText("데이터 가져오기");
    await fireEvent.input(input, { target: { value: " /old/lifeops " } });
    await fireEvent.click(screen.getByRole("button", { name: "가져오기" }));

    expect(mocks.importFromDir).toHaveBeenCalledWith("/old/lifeops");
    expect(await screen.findByText(/가져오기 준비 완료/)).toHaveTextContent("앱을 재시작하면 적용됩니다");
  });

  it("데스크탑 가져오기 실패를 표시하고 재시도 경로를 보존한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.importFromDir.mockRejectedValue(new Error("unsafe"));

    render(Settings);
    const input = await screen.findByLabelText("데이터 가져오기");
    await fireEvent.input(input, { target: { value: "/old/lifeops" } });
    await fireEvent.click(screen.getByRole("button", { name: "가져오기" }));

    expect(await screen.findByText(/데이터 가져오기를 준비하지 못했습니다: Error: unsafe/)).toBeInTheDocument();
    expect(input).toHaveValue("/old/lifeops");
  });

  it("데스크탑에서는 데이터 폴더를 열 수 있다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.openDataDir.mockResolvedValue(undefined);

    render(Settings);
    await fireEvent.click(await screen.findByRole("button", { name: "폴더 열기" }));

    expect(mocks.openDataDir).toHaveBeenCalledOnce();
  });

  it("자동시작 토글 실패 시 이전 상태로 롤백하고 오류를 표시한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(true);
    mocks.setAutostart.mockRejectedValue(new Error("권한 거부"));

    render(Settings);
    const checkbox = await screen.findByRole("checkbox", { name: "로그인 시 자동 시작 사용" });
    await waitFor(() => expect(checkbox).toBeChecked());
    await fireEvent.click(checkbox);

    await waitFor(() => expect(checkbox).toBeChecked());
    expect(await screen.findByRole("alert")).toHaveTextContent("자동 시작 설정을 변경하지 못했습니다.");
    expect(mocks.setAutostart).toHaveBeenCalledWith(false);
  });

  it("시스템 정보와 자동시작 조회 실패를 안전하게 표시한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockRejectedValue(new Error("offline"));
    mocks.getAutostart.mockRejectedValue(new Error("unsupported"));

    render(Settings);

    expect(await screen.findByText("시스템 정보를 불러오지 못했습니다.")).toBeInTheDocument();
    expect(await screen.findByText("자동 시작 상태를 불러오지 못했습니다.")).toBeInTheDocument();
  });

  it("비동기 조회 중 unmount되어도 늦은 결과를 안전하게 무시한다", async () => {
    let resolveInfo!: (value: typeof info) => void;
    let resolveAutostart!: (value: boolean) => void;
    mocks.desktop = true;
    mocks.getSystemInfo.mockReturnValue(new Promise((resolve) => { resolveInfo = resolve; }));
    mocks.getAutostart.mockReturnValue(new Promise((resolve) => { resolveAutostart = resolve; }));
    const unhandled = vi.fn();
    window.addEventListener("unhandledrejection", unhandled);

    const view = render(Settings);
    view.unmount();
    resolveInfo(info);
    resolveAutostart(true);
    await Promise.resolve();
    await Promise.resolve();

    expect(unhandled).not.toHaveBeenCalled();
    window.removeEventListener("unhandledrejection", unhandled);
  });

  it("접속 범위를 LAN으로 바꾸면 부분 patch를 저장하고 재시작 적용을 안내한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.putConfig.mockResolvedValue({ bind_scope: "lan", backup_dir: null, backup_keep: 7 });

    render(Settings);
    const toggle = await screen.findByRole("checkbox", { name: "같은 네트워크(LAN) 허용" });
    await fireEvent.click(toggle);

    expect(mocks.putConfig).toHaveBeenCalledWith({ bind_scope: "lan" });
    expect(await screen.findByText(/재시작하면 적용/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "지금 재시작" })).toBeInTheDocument();
  });

  it("저장 범위와 현재 실효 범위가 다르면 재진입 직후에도 재시작을 안내한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.getConfig.mockResolvedValue({ bind_scope: "lan", backup_dir: null, backup_keep: 7 });

    render(Settings);

    expect(await screen.findByText(/재시작하면 적용/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "지금 재시작" })).toBeInTheDocument();
  });

  it("저장 범위를 현재 실효 범위로 되돌리면 재시작 안내를 제거한다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getConfig.mockResolvedValue({ bind_scope: "lan", backup_dir: null, backup_keep: 7 });
    mocks.putConfig.mockResolvedValue({ bind_scope: "localhost", backup_dir: null, backup_keep: 7 });

    render(Settings);
    const toggle = await screen.findByRole("checkbox", { name: "같은 네트워크(LAN) 허용" });
    expect(await screen.findByText(/재시작하면 적용/)).toBeInTheDocument();
    await fireEvent.click(toggle);

    await waitFor(() => expect(screen.queryByText(/재시작하면 적용/)).not.toBeInTheDocument());
  });

  it("접속 범위 저장 실패 시 이전 값으로 롤백한다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.putConfig.mockRejectedValue(new Error("offline"));

    render(Settings);
    const toggle = await screen.findByRole("checkbox", { name: "같은 네트워크(LAN) 허용" });
    expect(toggle).not.toBeChecked();
    await fireEvent.click(toggle);

    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(await screen.findByText("접속 범위를 저장하지 못했습니다.")).toBeInTheDocument();
  });

  it("지금 백업 후 목록을 새로고침한다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.createBackup.mockResolvedValue({ name: "lifeops-new.zip", created_at: "", size: 10 });
    mocks.listBackups
      .mockResolvedValueOnce({ backup_dir: "/b", accessible: true, last_success: null, snapshots: [] })
      .mockResolvedValueOnce({
        backup_dir: "/b",
        accessible: true,
        last_success: "2026-07-14T00:00:00+09:00",
        snapshots: [{ name: "lifeops-new.zip", created_at: "", size: 10 }],
      });

    render(Settings);
    await fireEvent.click(await screen.findByRole("button", { name: "지금 백업" }));

    expect(mocks.createBackup).toHaveBeenCalledOnce();
    expect(await screen.findByText("lifeops-new.zip")).toBeInTheDocument();
  });

  it("브라우저에서는 목록만 보여주고 폴더 입력·복원·재시작을 숨긴다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.listBackups.mockResolvedValue({
      backup_dir: "/b",
      accessible: true,
      last_success: null,
      snapshots: [{ name: "lifeops-a.zip", created_at: "", size: 10 }],
    });

    render(Settings);
    expect(await screen.findByText("lifeops-a.zip")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /이 시점으로 복원/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "백업 폴더" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "지금 재시작" })).not.toBeInTheDocument();
  });

  it("브라우저에서 백업 설정 저장은 보존 개수만 부분 갱신한다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.putConfig.mockResolvedValue({ bind_scope: "localhost", backup_dir: null, backup_keep: 3 });

    render(Settings);
    const keep = await screen.findByRole("spinbutton", { name: "보존 개수" });
    await fireEvent.input(keep, { target: { value: "3" } });
    await fireEvent.click(screen.getByRole("button", { name: "설정 저장" }));

    expect(mocks.putConfig).toHaveBeenCalledWith({ backup_keep: 3 });
  });

  it("데스크탑 복원은 파일명을 확인 후 전달하고 재시작 안내를 표시한다", async () => {
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.listBackups.mockResolvedValue({
      backup_dir: "/b",
      accessible: true,
      last_success: null,
      snapshots: [{ name: "lifeops-a.zip", created_at: "2026-07-14", size: 10 }],
    });
    mocks.restoreSnapshot.mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(Settings);
    await fireEvent.click(await screen.findByRole("button", { name: "lifeops-a.zip 이 시점으로 복원" }));

    expect(mocks.restoreSnapshot).toHaveBeenCalledWith("lifeops-a.zip");
    expect(await screen.findByText(/복원 준비 완료/)).toBeInTheDocument();
  });

  it("늦은 초기 백업 응답이 백업 직후 새 목록을 덮어쓰지 않는다", async () => {
    let resolveInitial!: (value: { backup_dir: string; accessible: boolean; last_success: string | null; snapshots: never[] }) => void;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.listBackups
      .mockReturnValueOnce(new Promise((resolve) => { resolveInitial = resolve; }))
      .mockResolvedValueOnce({
        backup_dir: "/b",
        accessible: true,
        last_success: "2026-07-14T00:00:00+09:00",
        snapshots: [{ name: "lifeops-new.zip", created_at: "", size: 10 }],
      });
    mocks.createBackup.mockResolvedValue({ name: "lifeops-new.zip", created_at: "", size: 10 });

    render(Settings);
    await fireEvent.click(await screen.findByRole("button", { name: "지금 백업" }));
    expect(await screen.findByText("lifeops-new.zip")).toBeInTheDocument();
    resolveInitial({ backup_dir: "/b", accessible: true, last_success: null, snapshots: [] });
    await Promise.resolve();

    expect(screen.getByText("lifeops-new.zip")).toBeInTheDocument();
  });

  it("백업 설정 저장이 끝나기 전에는 즉시 백업과 복원을 시작하지 않는다", async () => {
    let resolvePut!: (value: { bind_scope: "localhost"; backup_dir: string | null; backup_keep: number }) => void;
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.listBackups.mockResolvedValue({
      backup_dir: "/b",
      accessible: true,
      last_success: null,
      snapshots: [{ name: "lifeops-a.zip", created_at: "", size: 10 }],
    });
    mocks.putConfig.mockReturnValue(new Promise((resolve) => { resolvePut = resolve; }));
    vi.spyOn(window, "confirm").mockReturnValue(true);

    render(Settings);
    await screen.findByText("lifeops-a.zip");
    await fireEvent.click(screen.getByRole("button", { name: "설정 저장" }));

    const backupButton = screen.getByRole("button", { name: "지금 백업" });
    const restoreButton = screen.getByRole("button", { name: "lifeops-a.zip 이 시점으로 복원" });
    expect(backupButton).toBeDisabled();
    expect(restoreButton).toBeDisabled();
    await fireEvent.click(backupButton);
    await fireEvent.click(restoreButton);
    expect(mocks.createBackup).not.toHaveBeenCalled();
    expect(mocks.restoreSnapshot).not.toHaveBeenCalled();
    expect(window.confirm).not.toHaveBeenCalled();

    resolvePut({ bind_scope: "localhost", backup_dir: null, backup_keep: 7 });
    await waitFor(() => expect(backupButton).not.toBeDisabled());
    expect(restoreButton).not.toBeDisabled();
  });

  it("백업 폴더 확인 필요 상태를 중립적으로 표시하면서 지금 백업 재시도는 허용한다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.listBackups.mockResolvedValue({
      backup_dir: "/missing/backups",
      accessible: false,
      last_success: "2026-07-14T12:34:56+09:00",
      snapshots: [],
    });
    mocks.createBackup.mockRejectedValue(new Error("still unavailable"));

    render(Settings);
    expect(await screen.findByRole("status", { name: "확인 필요" })).toBeInTheDocument();
    expect(screen.getByText(/아직 없거나 접근할 수 없습니다/)).toHaveAttribute("role", "status");
    expect(screen.getByText("2026-07-14T12:34:56+09:00")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    const backupButton = screen.getByRole("button", { name: "지금 백업" });
    expect(backupButton).not.toBeDisabled();
    await fireEvent.click(backupButton);

    expect(mocks.createBackup).toHaveBeenCalledOnce();
    expect(await screen.findByText(/백업에 실패했습니다/)).toBeInTheDocument();
  });

  it("접근 가능한 백업 폴더는 정상 상태 배지를 표시한다", async () => {
    mocks.getSystemInfo.mockResolvedValue(info);

    render(Settings);

    expect(await screen.findByRole("status", { name: "정상" })).toBeInTheDocument();
  });

  it("백업 작업이 진행 중이면 재시작 버튼과 바인드 토글을 차단한다", async () => {
    let resolveBackup!: (value: { name: string; created_at: string; size: number }) => void;
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.putConfig.mockResolvedValue({ bind_scope: "lan", backup_dir: null, backup_keep: 7 });
    mocks.createBackup.mockReturnValue(new Promise((resolve) => { resolveBackup = resolve; }));

    render(Settings);
    const toggle = await screen.findByRole("checkbox", { name: "같은 네트워크(LAN) 허용" });
    await fireEvent.click(toggle);
    const restartButton = await screen.findByRole("button", { name: "지금 재시작" });
    await fireEvent.click(screen.getByRole("button", { name: "지금 백업" }));

    expect(restartButton).toBeDisabled();
    expect(toggle).toBeDisabled();
    await fireEvent.click(restartButton);
    await fireEvent.click(toggle);
    expect(mocks.relaunchApp).not.toHaveBeenCalled();
    expect(mocks.putConfig).toHaveBeenCalledTimes(1);

    resolveBackup({ name: "lifeops-new.zip", created_at: "", size: 10 });
    await waitFor(() => expect(restartButton).not.toBeDisabled());
    expect(toggle).not.toBeDisabled();
  });

  it("가져오기 작업이 진행 중이면 기존 재시작 CTA를 실행하지 않는다", async () => {
    let resolveImport!: () => void;
    mocks.desktop = true;
    mocks.getSystemInfo.mockResolvedValue(info);
    mocks.getAutostart.mockResolvedValue(false);
    mocks.putConfig.mockResolvedValue({ bind_scope: "lan", backup_dir: null, backup_keep: 7 });
    mocks.importFromDir.mockReturnValue(new Promise<void>((resolve) => { resolveImport = resolve; }));

    render(Settings);
    await fireEvent.click(await screen.findByRole("checkbox", { name: "같은 네트워크(LAN) 허용" }));
    const restartButton = await screen.findByRole("button", { name: "지금 재시작" });
    const importInput = screen.getByRole("textbox", { name: "데이터 가져오기" });
    await fireEvent.input(importInput, { target: { value: "/old/lifeops" } });
    await fireEvent.click(screen.getByRole("button", { name: "가져오기" }));

    expect(restartButton).toBeDisabled();
    await fireEvent.click(restartButton);
    expect(mocks.relaunchApp).not.toHaveBeenCalled();

    resolveImport();
    await waitFor(() => expect(restartButton).not.toBeDisabled());
  });
});
