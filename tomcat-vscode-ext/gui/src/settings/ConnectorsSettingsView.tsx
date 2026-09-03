import { useEffect, useMemo, useState } from "react";

import type { ConnectorInput, ConnectorToolFilter, ConnectorToolView, ConnectorView } from "../../../src/shared/connectorsProtocol";
import type { SettingsIntent, SettingsStateSnapshot, VsCodeApiLike } from "../../../src/shared/settingsProtocol";

function send(vscodeApi: VsCodeApiLike<SettingsIntent>, type: SettingsIntent["type"], data?: unknown) {
  vscodeApi.postMessage({
    messageId: `connector-${type}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    type,
    ...(data === undefined ? {} : { data }),
  } as SettingsIntent);
}

function statusLabel(connector: ConnectorView): string {
  switch (connector.state) {
    case "connected": return "Connected";
    case "connecting": return "Connecting";
    case "needs_confirmation": return "Needs confirmation";
    case "needs_authorization": return "Authorization required";
    case "blocked": return "Blocked";
    case "disconnected": return "Disconnected";
    default: return "Failed";
  }
}

function statusClass(connector: ConnectorView): string {
  return `tc-connector-status tc-connector-status--${connector.state}`;
}

export function ConnectorsSettingsView({
  state,
  vscodeApi,
}: {
  state: SettingsStateSnapshot;
  vscodeApi: VsCodeApiLike<SettingsIntent>;
}) {
  const connectors = state.connectors ?? [];
  const [selected, setSelected] = useState<ConnectorView | null>(null);
  const [tools, setTools] = useState<ConnectorToolView[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [transport, setTransport] = useState<"stdio" | "http">("stdio");
  const [url, setUrl] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [name, setName] = useState("");
  const [authMode, setAuthMode] = useState<"oauth" | "bearer" | "none">("oauth");

  const [bearerToken, setBearerToken] = useState("");
  const [customHeaders, setCustomHeaders] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [envText, setEnvText] = useState("");
  const [scope, setScope] = useState<"workspace" | "user">("workspace");
  const [clientId, setClientId] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [busyAction, setBusyAction] = useState<string | null>(null);


  useEffect(() => {
    if (state.selectedConnector && state.selectedConnector === selected?.name) {
      setTools(state.connectorTools ?? []);
    }
  }, [state.connectorTools, state.selectedConnector, selected?.name]);
  useEffect(() => {
    if (!isSubmitting || !state.status || state.status === "Authorizing connector…") return;
    setIsSubmitting(false);
    if (state.status.includes("connected") || state.status === "Connector saved.") {
      setShowAdd(false);
    }
  }, [isSubmitting, state.status]);
  useEffect(() => {
    if (!selected) return;
    const fresh = connectors.find((connector) => connector.name === selected.name);
    if (fresh) setSelected(fresh);
  }, [connectors, selected?.name]);
  useEffect(() => {
    if (state.status && state.status !== "Authorizing connector…") setBusyAction(null);
  }, [state.status]);

  const groups = useMemo(() => {
    const order: ConnectorView["state"][] = ["connected", "connecting", "needs_confirmation", "needs_authorization", "failed", "disconnected", "blocked"];
    return order.map((group) => ({
      label: group === "needs_confirmation" ? "Needs confirmation" : group === "needs_authorization" ? "Authorization required" : group,
      items: connectors.filter((connector) => connector.state === group),
    })).filter((group) => group.items.length > 0);
  }, [connectors]);

  function openDetail(connector: ConnectorView) {
    setSelected(connector);
    setTools([]);
    send(vscodeApi, "listConnectorTools", { name: connector.name });
  }

  function toggleTool(tool: ConnectorToolView) {
    if (!selected) return;
    const next = tools.map((entry) => entry.modelName === tool.modelName
      ? { ...entry, enabled: !entry.enabled }
      : entry);
    setTools(next);
    const currentToolNames = new Set(tools.map((entry) => entry.rawName));
    const filter: ConnectorToolFilter = {
      include: Array.from(new Set([
        ...(selected.toolFilter?.include ?? []),
        ...next.filter((entry) => entry.enabled).map((entry) => entry.rawName),
      ])),
      exclude: Array.from(new Set([
        ...(selected.toolFilter?.exclude ?? []).filter((name) => !currentToolNames.has(name)),
        ...next.filter((entry) => !entry.enabled).map((entry) => entry.rawName),
      ])),
    };    send(vscodeApi, "setConnectorToolFilter", { name: selected.name, filter });
  }

  function submitAdd() {
    const trimmedName = name.trim();
    if (!trimmedName) return setFormError("Connector name is required.");
    if (transport === "http" && !/^https?:\/\//i.test(url.trim())) {
      return setFormError("Enter an HTTP(S) MCP URL.");
    }
    if (transport === "stdio" && !command.trim()) {
      return setFormError("Enter a command for a stdio connector.");
    }
    const headers = customHeaders.split("\n").reduce<Record<string, string>>((result, line) => {
      const separator = line.indexOf(":");
      if (separator > 0) {
        const key = line.slice(0, separator).trim();
        const value = line.slice(separator + 1).trim();
        if (key && value) result[key] = value;
      }
      return result;
    }, {});
    if (authMode === "bearer") {
      if (!bearerToken.trim()) return setFormError("Enter a bearer token.");
      headers.Authorization = `Bearer ${bearerToken.trim()}`;
    }
    const env = envText.split("\n").reduce<Record<string, string>>((result, line) => {
      const separator = line.indexOf("=");
      if (separator > 0) {
        const key = line.slice(0, separator).trim();
        if (key) result[key] = line.slice(separator + 1);
      }
      return result;
    }, {});
    const input: ConnectorInput = transport === "http"
      ? { name: trimmedName, type: "mcp", transport, url: url.trim(), headers, auth: authMode, oauth: authMode === "oauth" ? { clientId: clientId.trim() || undefined } : undefined, scope }
      : { name: trimmedName, type: "mcp", transport, command: command.trim(), args: args.trim() ? args.trim().split(/\s+/) : [], env, scope };
    send(vscodeApi, "addConnector", { connector: input });
    setIsSubmitting(true);
    setFormError(null);
  }

  return (
    <div className="tc-settings-shell">
      <aside className="tc-settings-shell__nav">
        <div className="tc-settings-shell__brand">Tomcat Settings</div>
        <button className="tc-settings-nav__item" onClick={() => send(vscodeApi, "settings.ready", { route: "models" })} type="button">Models</button>
        <button className="tc-settings-nav__item" disabled type="button">Sessions</button>
        <button className="tc-settings-nav__item" disabled type="button">Tools</button>
        <button className="tc-settings-nav__item tc-settings-nav__item--active" type="button">Connectors</button>
        <div className="tc-settings-shell__version">
          <div>Extension {state.extensionVersion ?? "unknown"}</div>
          <div>Serve {state.serverVersion ?? "unknown"}</div>
        </div>
      </aside>
      <main className="tc-settings-shell__content">
        <header className="tc-settings-shell__header">
          <div>
            <h1>Connectors</h1>
            <p>External capabilities. MCP is available now; CLI and A2A can use the same surface later.</p>
          </div>
          <button className="tc-button tc-button--secondary" onClick={() => setShowAdd(true)} type="button">+ Add Connector</button>
        </header>
        {state.error ? <div className="tc-banner tc-banner--warning">{state.error}</div> : null}
        {state.status ? <div className="tc-banner">{state.status}</div> : null}
        {groups.length === 0 ? (
          <section className="tc-empty-state"><h2>No connectors configured</h2><p>Add a Workspace connector to make external tools available on demand.</p></section>
        ) : groups.map((group) => (
          <section className="tc-settings-group" key={group.label}>
            <h2 className="tc-settings-group__title">{group.label}</h2>
            <div className="tc-connector-list">
              {group.items.map((connector) => (
                <button className="tc-connector-card" key={connector.name} onClick={() => openDetail(connector)} type="button">
                  <span className={statusClass(connector)} aria-hidden="true">●</span>
                  <span className="tc-connector-card__body">
                    <strong>{connector.name}</strong>
                    <span>{statusLabel(connector)} · {connector.toolCount} tools · {connector.transport}</span>
                  </span>
                  <span className="tc-connector-card__source">{connector.source}</span>
                  <span aria-hidden="true">›</span>
                </button>
              ))}
            </div>
          </section>
        ))}
      </main>

      {selected ? (
        <div className="tc-modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setSelected(null); }}>
          <section aria-label={`Configure ${selected.name}`} className="tc-modal tc-connector-modal" role="dialog">
            <button className="tc-modal__close" onClick={() => setSelected(null)} type="button">×</button>
            <h2>{selected.name}</h2>
            <p className="tc-connector-modal__status"><span className={statusClass(selected)}>●</span> {statusLabel(selected)} · {selected.source}</p>
            <div className="tc-settings-group">
              <h3>Source</h3><p>{selected.source} configuration</p>
            </div>
            <div className="tc-settings-group">
              <h3>Transport</h3><p>{selected.transport === "http" ? selected.url ?? "HTTP" : selected.command ?? "stdio"}</p>
            </div>
            {selected.transport === "http" ? <div className="tc-settings-group"><h3>Authentication</h3><p>{selected.auth === "oauth" || selected.oauthConfigured ? "OAuth 2.0" : selected.auth === "bearer" ? "Bearer token" : "None"}</p>{selected.oauthConfigured || selected.auth === "oauth" ? <div className="tc-button-row"><button className="tc-button tc-button--secondary" disabled={busyAction === "login"} onClick={() => { setBusyAction("login"); send(vscodeApi, "loginConnector", { name: selected.name }); }} type="button">{busyAction === "login" ? "Authorizing…" : "Login / Re-login"}</button>{busyAction === "login" ? <button className="tc-button tc-button--secondary" onClick={() => { send(vscodeApi, "cancelLoginConnector", { name: selected.name }); setBusyAction(null); }} type="button">Cancel</button> : null}<button className="tc-button tc-button--secondary" onClick={() => send(vscodeApi, "logoutConnector", { name: selected.name })} type="button">Logout</button></div> : null}</div> : null}            <div className="tc-settings-group">
              <h3>Tools ({tools.length})</h3><p>Control which tools this connector exposes to progressive discovery.</p>
              <div className="tc-connector-tools">{tools.map((tool) => <button className="tc-connector-tool" key={tool.modelName} onClick={() => toggleTool(tool)} type="button"><span>{tool.label}</span><span aria-label={tool.enabled ? "Enabled" : "Disabled"}>{tool.enabled ? "●" : "○"}</span></button>)}</div>
            </div>
            <footer className="tc-modal__footer">{selected.state === "needs_confirmation" || selected.state === "blocked" ? <button className="tc-button" onClick={() => send(vscodeApi, "trustConnector", { name: selected.name })} type="button">Trust</button> : null}{selected.state === "needs_confirmation" ? <button className="tc-button tc-button--secondary" onClick={() => send(vscodeApi, "denyConnector", { name: selected.name })} type="button">Deny</button> : null}<button className="tc-button tc-button--danger" onClick={() => { send(vscodeApi, "removeConnector", { name: selected.name }); setSelected(null); }} type="button">Remove</button><button className="tc-button tc-button--secondary" onClick={() => send(vscodeApi, "reloadConnector", { name: selected.name })} type="button">↻ Reload</button><button className="tc-button" onClick={() => setSelected(null)} type="button">Done</button></footer>          </section>
        </div>
      ) : null}

      {showAdd ? (
        <div className="tc-modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowAdd(false); }}>
          <section aria-label="Add Connector" className="tc-modal tc-connector-modal" role="dialog">
            <button className="tc-modal__close" onClick={() => setShowAdd(false)} type="button">×</button>
            <h2>Add Connector</h2><p>Configure once; the connector will be stored in the current Workspace.</p>
            <label>Name<input value={name} onChange={(event) => setName(event.target.value)} /></label>
            <div className="tc-connector-radio-row"><label><input checked type="radio" onChange={() => {}} /> MCP</label><label className="tc-muted"><input disabled type="radio" /> CLI (soon)</label><label className="tc-muted"><input disabled type="radio" /> A2A (soon)</label></div>
            <div className="tc-connector-radio-row"><label><input checked={scope === "workspace"} name="scope" onChange={() => setScope("workspace")} type="radio" /> Workspace</label><label><input checked={scope === "user"} name="scope" onChange={() => setScope("user")} type="radio" /> User</label></div>
            <div className="tc-connector-radio-row"><label><input checked={transport === "stdio"} name="transport" onChange={() => setTransport("stdio")} type="radio" /> stdio</label><label><input checked={transport === "http"} name="transport" onChange={() => setTransport("http")} type="radio" /> HTTP</label></div>
            {transport === "http" && authMode === "oauth" ? <label>OAuth client ID <span className="tc-muted">optional; dynamic registration is used when empty</span><input placeholder="Optional pre-registered client ID" value={clientId} onChange={(event) => setClientId(event.target.value)} /></label> : null}
            {transport === "http" ? <><label>URL<input placeholder="https://example.com/mcp" value={url} onChange={(event) => setUrl(event.target.value)} /></label><label>Authentication<select value={authMode} onChange={(event) => setAuthMode(event.target.value as "oauth" | "bearer" | "none")}><option value="oauth">OAuth 2.0</option><option value="bearer">Bearer token</option><option value="none">None</option></select></label>{authMode === "bearer" ? <label>Bearer token<input type="password" placeholder="Stored locally; never shown again" value={bearerToken} onChange={(event) => setBearerToken(event.target.value)} /></label> : null}<label>Custom headers <span className="tc-muted">one per line: Header: value</span><textarea rows={3} placeholder="X-Tenant: team-a" value={customHeaders} onChange={(event) => setCustomHeaders(event.target.value)} /></label><p className="tc-muted">OAuth authorization uses the server's standard metadata. Add will save the Workspace config and start authorization when OAuth is selected.</p></> : <><label>Command<input placeholder="npx" value={command} onChange={(event) => setCommand(event.target.value)} /></label><label>Args<input placeholder="-y @playwright/mcp" value={args} onChange={(event) => setArgs(event.target.value)} /></label><label>Environment <span className="tc-muted">one per line: KEY=value</span><textarea rows={3} placeholder="API_KEY=..." value={envText} onChange={(event) => setEnvText(event.target.value)} /></label></>}
            {formError ? <div className="tc-banner tc-banner--warning">{formError}</div> : null}
            <footer className="tc-modal__footer"><button className="tc-button tc-button--secondary" onClick={() => { if (isSubmitting) send(vscodeApi, "cancelLoginConnector", { name: name.trim() }); setIsSubmitting(false); setShowAdd(false); }} type="button">Cancel</button><button className="tc-button" disabled={isSubmitting} onClick={submitAdd} type="button">{isSubmitting ? "Connecting…" : "Add"}</button></footer>
          </section>
        </div>
      ) : null}
    </div>
  );
}
