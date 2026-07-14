<script lang="ts">
  import {
    createBackup,
    getConfig,
    getSystemInfo,
    listBackups,
    putConfig,
    type AppConfig,
    type BackupsList,
    type SystemInfo,
  } from "../api";
  import {
    getAutostart,
    importFromDir,
    isDesktop,
    openDataDir,
    relaunchApp,
    restoreSnapshot,
    setAutostart,
  } from "../tauri";

  const desktop = isDesktop();
  let mounted = true;
  let info = $state<SystemInfo | null>(null);
  let infoError = $state("");
  let config = $state<AppConfig | null>(null);
  let configError = $state("");
  let bindBusy = $state(false);
  let bindError = $state("");
  let backups = $state<BackupsList | null>(null);
  let backupBusy = $state(false);
  let backupSettingsBusy = $state(false);
  let backupError = $state("");
  let restoreBusy = $state("");
  let restoreHint = $state(false);
  let keepInput = $state(7);
  let backupDirInput = $state("");
  let autostart = $state(false);
  let autostartReady = $state(false);
  let autostartBusy = $state(false);
  let autostartError = $state("");
  let dataDirError = $state("");
  let importPath = $state("");
  let importBusy = $state(false);
  let importMessage = $state("");
  let importError = $state("");
  let configRequest = 0;
  let backupsRequest = 0;
  let restartBlocked = $derived(
    bindBusy || backupSettingsBusy || backupBusy || restoreBusy !== "" || importBusy || autostartBusy,
  );
  let restartRequired = $derived(
    info !== null && config !== null && info.bind_scope !== config.bind_scope,
  );

  $effect(() => () => {
    mounted = false;
    configRequest += 1;
    backupsRequest += 1;
  });

  $effect(() => {
    let active = true;
    void getSystemInfo()
      .then((value) => { if (active) info = value; })
      .catch(() => { if (active) infoError = "시스템 정보를 불러오지 못했습니다."; });
    return () => { active = false; };
  });

  $effect(() => {
    const request = ++configRequest;
    void getConfig()
      .then((value) => {
        if (mounted && request === configRequest) {
          config = value;
          keepInput = value.backup_keep;
          backupDirInput = value.backup_dir ?? "";
        }
      })
      .catch(() => {
        if (mounted && request === configRequest) configError = "설정을 불러오지 못했습니다.";
      });
  });

  $effect(() => {
    void refreshBackups();
  });

  $effect(() => {
    if (!desktop) return;
    let active = true;
    void getAutostart()
      .then((value) => {
        if (active) {
          autostart = value;
          autostartReady = true;
        }
      })
      .catch(() => {
        if (active) {
          autostartReady = false;
          autostartError = "자동 시작 상태를 불러오지 못했습니다.";
        }
      });
    return () => { active = false; };
  });

  async function refreshBackups() {
    const request = ++backupsRequest;
    try {
      const value = await listBackups();
      if (mounted && request === backupsRequest) backups = value;
    } catch {
      if (mounted && request === backupsRequest) backups = null;
    }
  }

  async function toggleBind(event: Event) {
    if (bindBusy || backupSettingsBusy || backupBusy || restoreBusy || importBusy || config === null) return;
    const previous = config;
    const next = (event.currentTarget as HTMLInputElement).checked ? "lan" : "localhost";
    const request = ++configRequest;
    config = { ...config, bind_scope: next };
    bindBusy = true;
    bindError = "";
    try {
      const saved = await putConfig({ bind_scope: next });
      if (mounted && request === configRequest) {
        config = saved;
      }
    } catch {
      if (mounted && request === configRequest) {
        config = previous;
        bindError = "접속 범위를 저장하지 못했습니다.";
      }
    } finally {
      if (mounted && request === configRequest) bindBusy = false;
    }
  }

  async function saveBackupSettings() {
    if (backupSettingsBusy || backupBusy || restoreBusy || bindBusy || !Number.isInteger(keepInput) || keepInput < 1) return;
    const request = ++configRequest;
    backupSettingsBusy = true;
    backupError = "";
    try {
      const patch: Partial<AppConfig> = { backup_keep: keepInput };
      if (desktop) patch.backup_dir = backupDirInput.trim() || null;
      const saved = await putConfig(patch);
      if (mounted && request === configRequest) config = saved;
      if (mounted) await refreshBackups();
    } catch {
      if (mounted && request === configRequest) backupError = "백업 설정을 저장하지 못했습니다.";
    } finally {
      if (mounted && request === configRequest) backupSettingsBusy = false;
    }
  }

  async function backupNow() {
    if (backupBusy || backupSettingsBusy || restoreBusy) return;
    backupBusy = true;
    backupError = "";
    try {
      await createBackup();
      if (mounted) await refreshBackups();
    } catch (error) {
      if (mounted) backupError = `백업에 실패했습니다: ${String(error)}`;
    } finally {
      if (mounted) backupBusy = false;
    }
  }

  async function restore(name: string) {
    if (!desktop || restoreBusy || backupSettingsBusy || backupBusy) return;
    if (!confirm(`${name} 시점으로 복원할까요? 재시작 후 현재 데이터를 대체합니다(복원 전 자동 백업).`)) return;
    restoreBusy = name;
    backupError = "";
    try {
      await restoreSnapshot(name);
      if (mounted) restoreHint = true;
    } catch (error) {
      if (mounted) backupError = `복원 준비에 실패했습니다: ${String(error)}`;
    } finally {
      if (mounted) restoreBusy = "";
    }
  }

  async function restartNow() {
    if (!desktop || restartBlocked) return;
    backupError = "";
    try {
      await relaunchApp();
    } catch (error) {
      if (mounted) backupError = `앱을 재시작하지 못했습니다: ${String(error)}`;
    }
  }

  async function toggleAutostart(event: Event) {
    if (autostartBusy) return;
    const previous = autostart;
    const next = (event.currentTarget as HTMLInputElement).checked;
    autostart = next;
    autostartBusy = true;
    autostartError = "";
    try {
      await setAutostart(next);
    } catch {
      if (mounted) {
        autostart = previous;
        autostartError = "자동 시작 설정을 변경하지 못했습니다.";
      }
    } finally {
      if (mounted) autostartBusy = false;
    }
  }

  async function revealDataDir() {
    if (!desktop) return;
    dataDirError = "";
    try {
      await openDataDir();
    } catch {
      if (mounted) dataDirError = "데이터 폴더를 열지 못했습니다.";
    }
  }

  async function doImport() {
    if (!desktop) return;
    const path = importPath.trim();
    if (!path || importBusy) return;
    importBusy = true;
    importMessage = "";
    importError = "";
    try {
      await importFromDir(path);
      if (mounted) {
        importMessage = "가져오기 준비 완료 — 앱을 재시작하면 적용됩니다.";
        importPath = "";
      }
    } catch (error) {
      if (mounted) importError = `데이터 가져오기를 준비하지 못했습니다: ${String(error)}`;
    } finally {
      if (mounted) importBusy = false;
    }
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
</script>

<section class="settings">
  <h2>설정</h2>
  {#if info}
    <div class="row">
      <span class="label">데이터 위치</span>
      <code>{info.data_dir}</code>
      {#if desktop}<button type="button" onclick={revealDataDir}>폴더 열기</button>{/if}
    </div>
    {#if dataDirError}<p class="error" role="alert">{dataDirError}</p>{/if}
    <div class="row"><span class="label">포트</span><code>{info.port}</code></div>
    <div class="row"><span class="label">현재 접속 범위</span><span>{info.bind_scope === "lan" ? "같은 네트워크(LAN)" : "내 기기만(localhost)"}</span></div>
    <div class="row">
      <span class="label">LAN 접속 주소</span>
      {#if info.lan_addrs.length > 0}
        <ul>{#each info.lan_addrs as address (address)}<li><code>{address}</code></li>{/each}</ul>
      {:else}<span>없음</span>{/if}
    </div>

    <h3>접속 범위</h3>
    <label class="row">
      <span class="label">같은 네트워크(LAN) 허용</span>
      <input type="checkbox" checked={config?.bind_scope === "lan"} disabled={bindBusy || backupSettingsBusy || backupBusy || restoreBusy !== "" || importBusy || config === null} onchange={toggleBind} />
    </label>
    <p class="hint">기본은 내 기기에서만(127.0.0.1) 접속됩니다. 허용하면 같은 네트워크의 다른 기기가 위 LAN 주소로 접속할 수 있습니다.</p>
    {#if configError}<p class="error" role="alert">{configError}</p>{/if}
    {#if bindError}<p class="error" role="alert">{bindError}</p>{/if}
    {#if restartRequired}
      <p class="success" role="status">재시작하면 적용됩니다.{#if desktop} <button type="button" onclick={restartNow} disabled={restartBlocked}>지금 재시작</button>{/if}</p>
    {/if}
    <details>
      <summary>외부(집 밖)에서 접속하려면?</summary>
      <p class="hint">LAN 허용은 신뢰하는 네트워크(집 등)에서만 켜세요. 외출 중 접속이 필요하면 Tailscale 같은 개인 VPN을 권장합니다. 공유기 포트포워딩은 인증이 없어 전 세계에 노출되므로 권장하지 않습니다.</p>
    </details>

    <h3>백업</h3>
    {#if desktop}
      <div class="row">
        <label class="label" for="backup-dir">백업 폴더</label>
        <input id="backup-dir" type="text" placeholder="비우면 기본(data/backups)" bind:value={backupDirInput} disabled={backupSettingsBusy || backupBusy || restoreBusy !== "" || bindBusy} />
      </div>
      <p class="hint">iCloud Drive·Dropbox 같은 동기화 폴더를 지정하면 자동으로 오프사이트 백업이 됩니다.</p>
    {:else if backups}
      <div class="row"><span class="label">백업 폴더</span><code>{backups.backup_dir}</code></div>
    {/if}
    <div class="row">
      <label class="label" for="backup-keep">보존 개수</label>
      <input id="backup-keep" type="number" min="1" step="1" bind:value={keepInput} disabled={backupSettingsBusy || backupBusy || restoreBusy !== "" || bindBusy} />
      <button type="button" onclick={saveBackupSettings} disabled={backupSettingsBusy || backupBusy || restoreBusy !== "" || bindBusy || !Number.isInteger(keepInput) || keepInput < 1}>설정 저장</button>
    </div>
    <div class="row">
      <span class="label">접근 상태</span>
      {#if backups}
        <span class={backups.accessible ? "success" : "hint"} role="status" aria-label={backups.accessible ? "정상" : "확인 필요"}>{backups.accessible ? "정상" : "확인 필요"}</span>
      {:else}
        <span>확인 중…</span>
      {/if}
      <button type="button" onclick={backupNow} disabled={backupBusy || backupSettingsBusy || restoreBusy !== ""}>지금 백업</button>
    </div>
    {#if backups?.last_success}
      <div class="row"><span class="label">마지막 백업 성공</span><span>{backups.last_success}</span></div>
    {/if}
    {#if backupError}<p class="error" role="alert">{backupError}</p>{/if}
    {#if restoreHint}
      <p class="success" role="status">복원 준비 완료 — 재시작하면 이 시점으로 복원됩니다.{#if desktop} <button type="button" onclick={restartNow} disabled={restartBlocked}>지금 재시작</button>{/if}</p>
    {/if}
    {#if backups}
      {#if !backups.accessible}
        <p class="hint" role="status">백업 폴더가 아직 없거나 접근할 수 없습니다: {backups.backup_dir}. 지금 백업으로 폴더 생성 또는 재시도를 할 수 있습니다.</p>
      {:else if backups.snapshots.length === 0}
        <p class="hint">백업이 아직 없습니다.</p>
      {:else}
        <ul class="backups">
          {#each backups.snapshots as snap (snap.name)}
            <li>
              <code>{snap.name}</code>
              <span>{snap.created_at || "시각 정보 없음"}</span>
              <span>{formatSize(snap.size)}</span>
              {#if desktop}
                <button type="button" aria-label={`${snap.name} 이 시점으로 복원`} onclick={() => restore(snap.name)} disabled={restoreBusy !== "" || backupSettingsBusy || backupBusy}>이 시점으로 복원</button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    {/if}

    {#if desktop}
      <h3>기타</h3>
      <label class="row">
        <span class="label">로그인 시 자동 시작</span>
        <span><input type="checkbox" checked={autostart} disabled={!autostartReady || autostartBusy} onchange={toggleAutostart} /> 사용</span>
      </label>
      <div class="row">
        <label class="label" for="import-path">데이터 가져오기</label>
        <input id="import-path" type="text" placeholder="기존 디렉터리 경로" bind:value={importPath} disabled={importBusy} />
        <button type="button" onclick={doImport} disabled={!importPath.trim() || importBusy}>가져오기</button>
      </div>
      <p class="hint">현재 데이터는 유지되며, 가져온 데이터는 다음 앱 시작 전에 안전하게 적용됩니다.</p>
    {/if}
  {:else if infoError}
    <p class="error" role="alert">{infoError}</p>
  {:else}
    <p>불러오는 중…</p>
  {/if}
  {#if desktop && autostartError}<p class="error" role="alert">{autostartError}</p>{/if}
  {#if desktop && importMessage}<p class="success" role="status">{importMessage}</p>{/if}
  {#if desktop && importError}<p class="error" role="alert">{importError}</p>{/if}
</section>
