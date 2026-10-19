/** Address book: saved recipients, encrypted with the wallet file. */

import { api } from "../api.js?v=7";
import { copyText, escapeHtml } from "../format.js?v=7";
import { confirmDialog, icon, openModal, setBusy, toast } from "../ui.js?v=7";

export function render(ctx) {
  const local = { chain: "all", query: "" };

  const contacts = () => ctx.state.wallet.contacts ?? [];
  const accounts = () => ctx.state.wallet.accounts;
  const accountFor = (chain) => accounts().find((item) => item.chain === chain);

  const filtered = () => {
    const query = local.query.toLowerCase();
    return contacts().filter((contact) => {
      if (local.chain !== "all" && contact.chain !== local.chain) return false;
      if (!query) return true;
      return (
        contact.name.toLowerCase().includes(query) ||
        contact.address.toLowerCase().includes(query) ||
        (contact.note ?? "").toLowerCase().includes(query)
      );
    });
  };

  const listMarkup = () => {
    const items = filtered();
    if (contacts().length === 0) {
      return `<div class="empty">
        <div class="mark">${icon("contacts")}</div>
        地址簿还是空的。<br />添加常用地址，转账时就不用反复核对长地址了。
      </div>`;
    }
    if (items.length === 0) return `<div class="empty">没有匹配的联系人。</div>`;
    return `<div class="contact-grid">${items.map(cardMarkup).join("")}</div>`;
  };

  const cardMarkup = (contact) => {
    const account = accountFor(contact.chain);
    return `<div class="contact-card" data-contact="${escapeHtml(contact.id)}">
      <div class="head">
        <span class="dot" style="--accent:${account?.accent ?? "var(--ink-3)"}"></span>
        <span class="name">${escapeHtml(contact.name)}</span>
        <span class="chip" style="margin-left:auto">${escapeHtml(account?.symbol ?? contact.chain)}</span>
      </div>
      <div class="addr">${escapeHtml(contact.address)}</div>
      ${contact.note ? `<div class="note">${escapeHtml(contact.note)}</div>` : ""}
      <div class="actions">
        <button class="btn btn-ghost btn-sm" data-action="copy" type="button">${icon("copy")}复制</button>
        <button class="btn btn-ghost btn-sm" data-action="send" type="button">${icon("send")}转账</button>
        <button class="btn btn-ghost btn-sm" data-action="edit" type="button">${icon("edit")}编辑</button>
        <button class="btn btn-ghost btn-sm" data-action="remove" type="button">${icon("trash")}删除</button>
      </div>
    </div>`;
  };

  const openEditor = (contact) => {
    const editing = Boolean(contact);
    const chains = accounts();
    const draft = contact ?? {
      name: "",
      chain: local.chain === "all" ? chains[0].chain : local.chain,
      address: "",
      note: "",
    };
    openModal(
      `<h2>${editing ? "编辑联系人" : "新建联系人"}</h2>
       <div class="stack-sm" style="margin-top:10px">
         <div class="field">
           <label for="contact-name">名称</label>
           <input class="input" id="contact-name" maxlength="40" placeholder="例如：交易所充值"
             value="${escapeHtml(draft.name)}" />
         </div>
         <div class="field">
           <label for="contact-chain">网络</label>
           <select class="select" id="contact-chain">
             ${chains
               .map(
                 (account) =>
                   `<option value="${account.chain}" ${account.chain === draft.chain ? "selected" : ""}>${escapeHtml(
                     account.name,
                   )} · ${escapeHtml(account.symbol)}</option>`,
               )
               .join("")}
           </select>
         </div>
         <div class="field">
           <label for="contact-address">地址</label>
           <input class="input mono" id="contact-address" spellcheck="false" autocomplete="off"
             placeholder="粘贴完整地址" value="${escapeHtml(draft.address)}" />
           <p class="hint">保存时会按所选网络的格式校验地址。</p>
         </div>
         <div class="field">
           <label for="contact-note">备注（可选）</label>
           <input class="input" id="contact-note" maxlength="80" value="${escapeHtml(draft.note ?? "")}" />
         </div>
         <p class="error-text" id="contact-error" hidden></p>
       </div>
       <div class="modal-foot">
         <button class="btn btn-ghost" data-close type="button">取消</button>
         <button class="btn btn-primary" id="contact-save" type="button">${
           editing ? "保存修改" : "添加联系人"
         }</button>
       </div>`,
      {
        onMount(element, close) {
          const button = element.querySelector("#contact-save");
          const error = element.querySelector("#contact-error");
          const fail = (message) => {
            error.hidden = false;
            error.textContent = message;
          };
          button.addEventListener("click", async () => {
            const name = element.querySelector("#contact-name").value.trim();
            const chain = element.querySelector("#contact-chain").value;
            const address = element.querySelector("#contact-address").value.trim();
            const note = element.querySelector("#contact-note").value.trim();
            if (!name) return fail("请填写联系人名称");
            if (!address) return fail("请填写地址");
            setBusy(button, true, "保存中…");
            try {
              const list = await api.saveContact({ id: contact?.id ?? null, name, chain, address, note });
              ctx.state.wallet = { ...ctx.state.wallet, contacts: list };
              close();
              paint();
              toast(editing ? "联系人已更新" : "联系人已添加", "success");
            } catch (failure) {
              setBusy(button, false);
              fail(failure.message);
            }
          });
        },
      },
    );
  };

  const removeContact = async (contact) => {
    const confirmed = await confirmDialog({
      title: "删除联系人",
      danger: true,
      confirmText: "删除",
      body: `<p>确定要删除「<strong>${escapeHtml(contact.name)}</strong>」吗？</p>
        <p class="hint">只会从本机地址簿移除，不影响链上资产。</p>`,
    });
    if (!confirmed) return;
    try {
      const list = await api.deleteContact(contact.id);
      ctx.state.wallet = { ...ctx.state.wallet, contacts: list };
      paint();
      toast("联系人已删除", "success");
    } catch (error) {
      toast(error.message, "error");
    }
  };

  const copyContact = async (contact) => {
    const ok = await copyText(contact.address);
    toast(ok ? "地址已复制" : "复制失败", ok ? "success" : "error");
  };

  let paint = () => {};

  return {
    html: `<section class="view">
      <div class="page-head">
        <div>
          <h1>地址簿</h1>
          <div class="sub">常用地址与钱包文件一起加密保存在本机。</div>
        </div>
        <button class="btn btn-primary" data-action="add" type="button">${icon("plus")}新建联系人</button>
      </div>

      <div class="card card-pad">
        <div class="row-between" style="flex-wrap:wrap;gap:10px;margin-bottom:14px">
          <div class="tabs" id="contact-tabs">
            <button class="tab" type="button" data-chain="all" aria-selected="true">全部</button>
            ${accounts()
              .map(
                (account) =>
                  `<button class="tab" type="button" data-chain="${account.chain}" aria-selected="false"><span class="dot" style="--accent:${account.accent}"></span>${escapeHtml(
                    account.symbol,
                  )}</button>`,
              )
              .join("")}
          </div>
          <input class="input" id="contact-search" style="max-width:240px" placeholder="搜索名称或地址"
            autocomplete="off" spellcheck="false" />
        </div>
        <div id="contact-list">${listMarkup()}</div>
      </div>
    </section>`,

    mount(root) {
      const list = root.querySelector("#contact-list");
      const tabs = root.querySelector("#contact-tabs");
      const search = root.querySelector("#contact-search");

      paint = () => {
        list.innerHTML = listMarkup();
      };

      tabs.addEventListener("click", (event) => {
        const button = event.target.closest("[data-chain]");
        if (!button) return;
        local.chain = button.dataset.chain;
        tabs
          .querySelectorAll("[data-chain]")
          .forEach((item) => item.setAttribute("aria-selected", String(item === button)));
        paint();
      });

      search.addEventListener("input", () => {
        local.query = search.value.trim();
        paint();
      });

      list.addEventListener("click", (event) => {
        const button = event.target.closest("[data-action]");
        if (!button) return;
        const card = button.closest("[data-contact]");
        const contact = contacts().find((item) => item.id === card?.dataset.contact);
        if (!contact) return;
        switch (button.dataset.action) {
          case "copy":
            void copyContact(contact);
            break;
          case "send":
            ctx.go("send", { chain: contact.chain, to: contact.address });
            break;
          case "edit":
            openEditor(contact);
            break;
          case "remove":
            void removeContact(contact);
            break;
          default:
            break;
        }
      });

      root.querySelector('[data-action="add"]')?.addEventListener("click", () => openEditor(null));
    },
  };
}
