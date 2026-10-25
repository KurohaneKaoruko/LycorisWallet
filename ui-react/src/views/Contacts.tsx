/** Address book: saved recipients, encrypted with the wallet file.
 *  Ported from ui/js/views/contacts.js. */

import { useMemo, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useTabScroll, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
import type { ChainId, Contact } from "../types";

interface Draft {
  id: string | null;
  name: string;
  chain: ChainId;
  address: string;
  note: string;
}

function EditorDialog({
  draft: initial,
  chains,
  onClose,
  onSaved,
}: {
  draft: Draft;
  chains: Array<{ chain: string; name: string; symbol: string }>;
  onClose: () => void;
  onSaved: (list: Contact[]) => void;
}) {
  const [draft, setDraft] = useState(initial);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const save = async (draft: {
    id: string | null;
    name: string;
    chain: ChainId;
    address: string;
    note: string;
  }) => {
    if (!draft.name.trim()) return setError("请填写联系人名称");
    if (!draft.address.trim()) return setError("请填写地址");
    setBusy(true);
    setError("");
    try {
      const list = (await api.saveContact({
        id: draft.id ?? undefined,
        name: draft.name.trim(),
        chain: draft.chain,
        address: draft.address.trim(),
        note: draft.note.trim(),
      })) as Contact[];
      onSaved(list);
      onClose();
      toast(draft.id ? "联系人已更新" : "联系人已添加", "success");
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <Modal title={draft.id ? "编辑联系人" : "新建联系人"} onClose={onClose}>
      <div className="stack-sm" style={{ marginTop: 10 }}>
        <div className="field">
          <label htmlFor="contact-name">名称</label>
          <input
            className="input"
            id="contact-name"
            maxLength={40}
            placeholder="例如：交易所充值"
            value={draft.name}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          />
        </div>
        <div className="field">
          <label htmlFor="contact-chain">网络</label>
          <select
            className="select"
            id="contact-chain"
            value={draft.chain}
            onChange={(event) => setDraft({ ...draft, chain: event.target.value as ChainId })}
          >
            {chains.map((account) => (
              <option key={account.chain} value={account.chain}>
                {account.name} · {account.symbol}
              </option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor="contact-address">地址</label>
          <input
            className="input mono"
            id="contact-address"
            spellCheck={false}
            autoComplete="off"
            placeholder="粘贴完整地址"
            value={draft.address}
            onChange={(event) => setDraft({ ...draft, address: event.target.value })}
          />
          <p className="hint">保存时会按所选网络的格式校验地址。</p>
        </div>
        <div className="field">
          <label htmlFor="contact-note">备注（可选）</label>
          <input
            className="input"
            id="contact-note"
            maxLength={80}
            value={draft.note}
            onChange={(event) => setDraft({ ...draft, note: event.target.value })}
          />
        </div>
        {error ? <p className="error-text">{error}</p> : null}
      </div>
      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          取消
        </button>
        <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void save(draft)}>
          {busy ? "保存中…" : draft.id ? "保存修改" : "添加联系人"}
        </button>
      </div>
    </Modal>
  );
}

export function ContactsView() {
  const { wallet, setWallet, go } = useSession();
  const toast = useToast();
  const { confirm } = useDialogs();
  const tabsRef = useTabScroll<HTMLDivElement>();
  const [chainTab, setChainTab] = useState("all");
  const [query, setQuery] = useState("");
  const [editor, setEditor] = useState<Draft | null>(null);

  const contacts = wallet?.contacts ?? [];
  const accounts = wallet?.accounts ?? [];

  const filtered = useMemo(() => {
    const q = query.toLowerCase();
    return contacts.filter((contact) => {
      if (chainTab !== "all" && contact.chain !== chainTab) return false;
      if (!q) return true;
      return (
        contact.name.toLowerCase().includes(q) ||
        contact.address.toLowerCase().includes(q) ||
        (contact.note ?? "").toLowerCase().includes(q)
      );
    });
  }, [contacts, chainTab, query]);

  const accountFor = (chain: string) => accounts.find((item) => item.chain === chain);

  const removeContact = async (contact: Contact) => {
    const ok = await confirm({
      title: "删除联系人",
      danger: true,
      confirmText: "删除",
      body: `<p>确定要删除「<strong>${contact.name}</strong>」吗？</p><p class="hint">只会从本机地址簿移除，不影响链上资产。</p>`,
    });
    if (!ok) return;
    try {
      const list = (await api.deleteContact(contact.id)) as Contact[];
      if (wallet) setWallet({ ...wallet, contacts: list });
      toast("联系人已删除", "success");
    } catch (error) {
      toast((error as Error).message, "error");
    }
  };

  const copyContact = async (contact: Contact) => {
    const ok = await copyText(contact.address);
    toast(ok ? "地址已复制" : "复制失败", ok ? "success" : "error");
  };

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>地址簿</h1>
          <div className="sub">常用地址与钱包文件一起加密保存在本机。</div>
        </div>
        <button
          className="btn btn-primary"
          type="button"
          onClick={() => {
            if (accounts.length === 0) return;
            setEditor({
              id: null,
              name: "",
              chain: chainTab === "all" ? accounts[0].chain : (chainTab as ChainId),
              address: "",
              note: "",
            });
          }}
        >
          <Icon name="plus" />
          新建联系人
        </button>
      </div>

      <div className="card card-pad">
        <div className="row-between" style={{ flexWrap: "wrap", gap: 10, marginBottom: 14 }}>
          <div className="tabs" id="contact-tabs" ref={tabsRef}>
            <button
              className="tab"
              type="button"
              data-chain="all"
              aria-selected={chainTab === "all"}
              onClick={() => setChainTab("all")}
            >
              全部
            </button>
            {accounts.map((account) => (
              <button
                key={account.chain}
                className="tab"
                type="button"
                data-chain={account.chain}
                aria-selected={chainTab === account.chain}
                onClick={() => setChainTab(account.chain)}
              >
                <span className="dot" style={{ ["--accent" as string]: account.accent }} />
                {account.symbol}
              </button>
            ))}
          </div>
          <input
            className="input"
            id="contact-search"
            style={{ maxWidth: 240 }}
            placeholder="搜索名称或地址"
            autoComplete="off"
            spellCheck={false}
            value={query}
            onChange={(event) => setQuery(event.target.value.trim())}
          />
        </div>
        <div id="contact-list">
          {contacts.length === 0 ? (
            <div className="empty">
              <div className="mark">
                <Icon name="contacts" />
              </div>
              地址簿还是空的。
              <br />
              添加常用地址，转账时就不用反复核对长地址了。
            </div>
          ) : filtered.length === 0 ? (
            <div className="empty">没有匹配的联系人。</div>
          ) : (
            <div className="contact-grid">
              {filtered.map((contact) => {
                const account = accountFor(contact.chain);
                return (
                  <div className="contact-card" key={contact.id} data-contact={contact.id}>
                    <div className="head">
                      <span className="dot" style={{ ["--accent" as string]: account?.accent ?? "var(--ink-3)" }} />
                      <span className="name">{contact.name}</span>
                      <span className="chip" style={{ marginLeft: "auto" }}>
                        {account?.symbol ?? contact.chain}
                      </span>
                    </div>
                    <div className="addr">{contact.address}</div>
                    {contact.note ? <div className="note">{contact.note}</div> : null}
                    <div className="actions">
                      <button className="btn btn-ghost btn-sm" type="button" onClick={() => void copyContact(contact)}>
                        <Icon name="copy" />
                        复制
                      </button>
                      <button
                        className="btn btn-ghost btn-sm"
                        type="button"
                        onClick={() => go("send", { chain: contact.chain, to: contact.address })}
                      >
                        <Icon name="send" />
                        转账
                      </button>
                      <button
                        className="btn btn-ghost btn-sm"
                        type="button"
                        onClick={() =>
                          setEditor({
                            id: contact.id,
                            name: contact.name,
                            chain: contact.chain,
                            address: contact.address,
                            note: contact.note ?? "",
                          })
                        }
                      >
                        <Icon name="edit" />
                        编辑
                      </button>
                      <button className="btn btn-ghost btn-sm" type="button" onClick={() => void removeContact(contact)}>
                        <Icon name="trash" />
                        删除
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {editor ? (
        <EditorDialog
          draft={editor}
          chains={accounts.map((a) => ({ chain: a.chain, name: a.name, symbol: a.symbol }))}
          onClose={() => setEditor(null)}
          onSaved={(list) => {
            if (wallet) setWallet({ ...wallet, contacts: list });
          }}
        />
      ) : null}
    </section>
  );
}
