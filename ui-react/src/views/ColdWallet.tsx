/** U盘冷钱包卡片：把加密保险柜搬到可移动磁盘上，拔盘即锁定。
 *
 * 放在设置页的「安全」卡片之后。核心动作只有两个：
 *  - 启用：选一块可移动磁盘，验密码后把本机 vault.json 拷过去并切换过去；
 *  - 停用/恢复：把U盘上的保险柜拉回本机（验密码），或直接切回本机副本。
 *
 * 监听 `lycoris://cold-drive-lost` 事件：拔盘时后端已经锁定会话，这里负责把
 * 前端状态拉回解锁页并提示原因。 */

import { useCallback, useEffect, useState } from "react";

import { api, listen } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import type { ColdStatus, UsbDrive } from "../types";

function sizeText(bytes: number | null): string {
  if (bytes == null) return "容量未知";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}

export function ColdWalletCard() {
  const { lockNow, refreshStatus } = useSession();
  const toast = useToast();
  const { askPassword, confirm } = useDialogs();

  const [cold, setCold] = useState<ColdStatus | null>(null);
  const [drives, setDrives] = useState<UsbDrive[] | null>(null);
  const [pickOpen, setPickOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      setCold(await api.coldStatus());
    } catch {
      setCold(null);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  // 拔盘：后端已锁定，前端回到解锁页并说明原因。
  useEffect(
    () =>
      listen("lycoris://cold-drive-lost", () => {
        void refreshStatus();
        void lockNow();
        toast("检测到U盘已拔出，钱包已立即锁定。", "info");
      }),
    [refreshStatus, lockNow, toast],
  );

  const openPicker = async () => {
    setPickOpen(true);
    setDrives(null);
    try {
      setDrives(await api.usbDrives());
    } catch (e) {
      toast((e as Error).message || "无法枚举可移动磁盘", "error");
      setDrives([]);
    }
  };

  const enable = async (mount: string) => {
    const password = await askPassword({
      title: "启用U盘冷钱包",
      description:
        "将验证密码，然后把本机加密保险柜复制到该U盘并改从U盘加载。之后拔出U盘会立即锁定钱包。",
      confirmText: "启用",
    });
    if (!password) return;
    setBusy(true);
    try {
      const next = await api.coldEnable(mount, password, true);
      setCold(next);
      setPickOpen(false);
      await refreshStatus();
      toast("已启用U盘冷钱包：密钥文件现在存放在U盘上。", "success");
    } catch (e) {
      toast((e as Error).message || "启用失败", "error");
    } finally {
      setBusy(false);
    }
  };

  const disable = async (pull: boolean) => {
    const password = await askPassword({
      title: pull ? "恢复本机副本" : "停用U盘冷钱包",
      description: pull
        ? "将验证密码，把U盘上的保险柜复制回本机，并改回从本机加载。"
        : "将改回从本机的保险柜文件加载（若本机没有副本，请先选择恢复）。",
      confirmText: pull ? "恢复" : "停用",
    });
    if (!password) return;
    setBusy(true);
    try {
      const next = await api.coldDisable(pull, password);
      setCold(next);
      await refreshStatus();
      toast(pull ? "已恢复本机副本。" : "已停用U盘冷钱包。", "success");
    } catch (e) {
      toast((e as Error).message || "操作失败", "error");
    } finally {
      setBusy(false);
    }
  };

  const restoreFromScan = async () => {
    setBusy(true);
    try {
      const list = await api.usbDrives();
      const candidates = list.filter((d) => d.hasWallet);
      if (candidates.length === 0) {
        toast("没有找到带冷钱包的U盘，请确认已插入。", "info");
        return;
      }
      const password = await askPassword({
        title: "从U盘恢复",
        description: "将验证密码，把U盘上的保险柜复制回本机，并改回从本机加载。",
        confirmText: "恢复",
      });
      if (!password) return;
      let last: ColdStatus | null = null;
      for (const drive of candidates) {
        last = await api.coldDisable(true, password, drive.mount);
      }
      if (last) setCold(last);
      await refreshStatus();
      toast("已从U盘恢复本机副本。", "success");
    } catch (e) {
      toast((e as Error).message || "恢复失败", "error");
    } finally {
      setBusy(false);
    }
  };

  const deleteLocal = async () => {
    const ok = await confirm({
      title: "删除本机副本",
      body: "本机的加密保险柜文件将被删除，密钥只保留在U盘上。请确认U盘工作正常且已妥善保管。",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      const next = await api.coldDeleteLocal();
      setCold(next);
      await refreshStatus();
      toast("已删除本机副本：密钥文件现在只在U盘上。", "success");
    } catch (e) {
      toast((e as Error).message || "删除失败", "error");
    }
  };

  if (!cold) return null;
  const active = cold.coldActive;

  return (
    <div className="card card-pad">
      <div className="section-title">
        <h2>U盘冷钱包</h2>
      </div>
      {active ? (
        <>
          <div className="note note-ok" style={{ marginBottom: 12 }}>
            <Icon name="usb" />
            <div>
              冷钱包模式已启用：保险柜文件存放在U盘上，本机
              {cold.localExists ? "保留一份副本" : "没有副本（仅U盘）"}。
              拔出U盘会立即锁定钱包。
            </div>
          </div>
          <div className="row" style={{ gap: 10, flexWrap: "wrap" }}>
            <button className="btn btn-ghost btn-sm" type="button" disabled={busy} onClick={() => void disable(true)}>
              <Icon name="download" />
              恢复本机副本
            </button>
            {!cold.localExists ? null : (
              <button className="btn btn-ghost btn-sm" type="button" disabled={busy} onClick={() => void disable(false)}>
                <Icon name="usb" />
                切回本机副本
              </button>
            )}
            {cold.localExists ? (
              <button className="btn btn-danger btn-sm" type="button" disabled={busy} onClick={() => void deleteLocal()}>
                <Icon name="trash" />
                删除本机副本
              </button>
            ) : null}
          </div>
        </>
      ) : (
        <>
          <div className="setting-row">
            <div className="text">
              <div className="name">把保险柜搬到U盘</div>
              <div className="desc">
                加密密钥文件只存U盘（本机默认保留副本）：拔出U盘立即锁定，插入U盘并输入密码才能解锁。适合偶尔签名、平时密钥不落盘的用法。
              </div>
            </div>
            <button className="btn btn-ghost btn-sm" type="button" disabled={busy} onClick={() => void openPicker()}>
              <Icon name="usb" />
              启用
            </button>
          </div>
          <div className="setting-row">
            <div className="text">
              <div className="name">从U盘恢复</div>
              <div className="desc">
                已在别的电脑启用过冷钱包？插入U盘后点这里，扫描带冷钱包标记的U盘并恢复到本机。
              </div>
            </div>
            <button className="btn btn-ghost btn-sm" type="button" disabled={busy} onClick={() => void restoreFromScan()}>
              <Icon name="download" />
              恢复
            </button>
          </div>
          <p className="hint" style={{ marginTop: 10 }}>
            提示：冷钱包模式下每次交易签名都需要插入U盘。U盘丢失不会泄露密钥（文件是加密的），但请像助记词一样妥善保管。
          </p>
        </>
      )}

      {pickOpen ? (
        <Modal title="选择冷钱包U盘" onClose={() => setPickOpen(false)}>
          {drives === null ? (
            <p className="hint">正在扫描可移动磁盘…</p>
          ) : drives.length === 0 ? (
            <p className="hint">没有找到可移动磁盘。请插入U盘后重试。</p>
          ) : (
            <div className="stack-sm">
              {drives.map((drive) => (
                <button
                  key={drive.mount}
                  className="more-item"
                  type="button"
                  disabled={busy}
                  onClick={() => void enable(drive.mount)}
                >
                  <Icon name="usb" />
                  <span>
                    {drive.label}
                    <span className="hint" style={{ display: "block" }}>
                      {sizeText(drive.totalBytes)} · 剩余 {sizeText(drive.freeBytes)}
                      {drive.hasWallet ? " · 检测到冷钱包" : ""}
                    </span>
                  </span>
                  <Icon name="chevronRight" />
                </button>
              ))}
            </div>
          )}
          <div className="modal-foot">
            <button className="btn btn-ghost" type="button" onClick={() => setPickOpen(false)}>
              取消
            </button>
          </div>
        </Modal>
      ) : null}
    </div>
  );
}
