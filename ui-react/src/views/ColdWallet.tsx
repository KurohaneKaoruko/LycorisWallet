/** U盘冷钱包卡片：U盘上放一个独立的钱包容器，与本机钱包互不干扰。
 *
 * 放在设置页的「安全」卡片之后。核心语义：
 *  - 启用/打开：选一块可移动磁盘，U盘上已有钱包就直接打开（用它自己的密码）；
 *    没有就把本机钱包复制一份过去作为初始内容（本机原件绝不动）。
 *  - 拔盘：只结束U盘钱包会话，本机钱包立即可正常解锁使用，其它功能不受任何影响。
 *  - 停用：切回本机钱包容器；U盘上的钱包原样保留。
 *  - 导入：换电脑时把U盘钱包拉回本机（显式覆盖操作，需确认）。
 *
 * 监听 `lycoris://cold-drive-lost` 事件：拔盘时后端已经结束U盘会话，这里负责把
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

  // 拔盘：后端已结束U盘会话（本机钱包不受影响），前端回到解锁页说明原因。
  useEffect(
    () =>
      listen("lycoris://cold-drive-lost", () => {
        void refreshStatus();
        void lockNow();
        toast("检测到U盘已拔出：U盘钱包会话已结束，本机钱包不受影响。", "info");
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

  const enable = async (drive: UsbDrive) => {
    const fresh = !drive.hasWallet;
    const password = await askPassword({
      title: fresh ? "启用U盘钱包" : "打开U盘钱包",
      description: fresh
        ? "将把本机钱包复制一份到该U盘作为初始内容（本机钱包原样保留），之后改用U盘上的钱包。两边从此独立演化，拔出U盘只结束U盘会话。"
        : "该U盘上已有一个钱包，将验证它的密码并打开。期间本机钱包保持原样，拔出U盘随时可回到本机钱包。",
      confirmText: fresh ? "启用" : "打开",
    });
    if (!password) return;
    setBusy(true);
    try {
      const next = await api.coldEnable(drive.mount, password);
      setCold(next);
      setPickOpen(false);
      await refreshStatus();
      toast(fresh ? "已启用U盘钱包：本机钱包保持原样。" : "已打开U盘钱包。", "success");
    } catch (e) {
      toast((e as Error).message || "操作失败", "error");
    } finally {
      setBusy(false);
    }
  };

  const disable = async () => {
    const ok = await confirm({
      title: "切换回本机钱包",
      body: "将结束U盘钱包会话并锁定，解锁后回到本机钱包。U盘上的钱包原样保留，随时可以重新打开。",
      confirmText: "切换回本机",
    });
    if (!ok) return;
    setBusy(true);
    try {
      const next = await api.coldDisable(false);
      setCold(next);
      await refreshStatus();
      toast("已回到本机钱包：U盘上的钱包已保留。", "success");
    } catch (e) {
      toast((e as Error).message || "操作失败", "error");
    } finally {
      setBusy(false);
    }
  };

  const importFromDrive = async (mount: string, label: string) => {
    const password = await askPassword({
      title: "从U盘导入钱包",
      description: "将验证U盘钱包的密码，把它的数据复制到本机（覆盖本机钱包文件）。请确认不再需要本机当前的钱包数据。",
      confirmText: "导入并覆盖",
    });
    if (!password) return;
    setBusy(true);
    try {
      await api.coldDisable(true, password, mount);
      setCold(await api.coldStatus());
      await refreshStatus();
      toast(`已从 ${label} 导入钱包到本机。`, "success");
    } catch (e) {
      toast((e as Error).message || "导入失败", "error");
    } finally {
      setBusy(false);
    }
  };

  if (!cold) return null;
  const active = cold.coldActive;

  return (
    <div className="card card-pad">
      <div className="section-title">
        <h2>U盘钱包</h2>
      </div>
      {active ? (
        <>
          <div className="note note-ok" style={{ marginBottom: 12 }}>
            <Icon name="usb" />
            <div>
              当前使用的是U盘上的钱包。拔出U盘只会结束U盘会话，本机钱包随时可以照常解锁，其它功能不受影响。
            </div>
          </div>
          <div className="row" style={{ gap: 10, flexWrap: "wrap" }}>
            <button className="btn btn-ghost btn-sm" type="button" disabled={busy} onClick={() => void disable()}>
              <Icon name="download" />
              切换回本机钱包
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="setting-row">
            <div className="text">
              <div className="name">打开U盘上的钱包</div>
              <div className="desc">
                在U盘上放一个独立的钱包：U盘已有钱包就直接打开（用它自己的密码）；没有钱包则把本机钱包复制一份过去，本机原件绝不动。拔出U盘只结束U盘会话，本机钱包与其它功能完全不受影响。
              </div>
            </div>
            <button className="btn btn-ghost btn-sm" type="button" disabled={busy} onClick={() => void openPicker()}>
              <Icon name="usb" />
              启用 / 打开
            </button>
          </div>
          <p className="hint" style={{ marginTop: 10 }}>
            提示：U盘钱包与本机钱包是两个独立容器，可以有不同的密码和数据。U盘丢失不影响本机钱包；拾到U盘的人也无法读出内容（文件是加密的）。
          </p>
        </>
      )}

      {pickOpen ? (
        <Modal title="选择U盘" onClose={() => setPickOpen(false)}>
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
                  onClick={() => void enable(drive)}
                >
                  <Icon name="usb" />
                  <span>
                    {drive.label}
                    <span className="hint" style={{ display: "block" }}>
                      {sizeText(drive.totalBytes)} · 剩余 {sizeText(drive.freeBytes)}
                      {drive.hasWallet ? " · 检测到U盘钱包" : " · 将复制本机钱包过去"}
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

      {!active ? (
        <div className="setting-row" style={{ marginTop: 8 }}>
          <div className="text">
            <div className="name">从U盘导入到本机</div>
            <div className="desc">
              换电脑或想把U盘钱包的数据拉回本机时使用：会覆盖本机钱包文件，需要验证U盘钱包的密码。
            </div>
          </div>
          <button
            className="btn btn-ghost btn-sm"
            type="button"
            disabled={busy}
            onClick={async () => {
              try {
                const list = await api.usbDrives();
                const candidates = list.filter((d) => d.hasWallet);
                if (candidates.length === 0) {
                  toast("没有找到带钱包的U盘，请确认已插入。", "info");
                  return;
                }
                if (candidates.length === 1) {
                  await importFromDrive(candidates[0].mount, candidates[0].label);
                  return;
                }
                setPickOpen(false);
                for (const drive of candidates) {
                  await importFromDrive(drive.mount, drive.label);
                }
              } catch (e) {
                toast((e as Error).message || "导入失败", "error");
              }
            }}
          >
            <Icon name="download" />
            导入
          </button>
        </div>
      ) : null}
    </div>
  );
}
