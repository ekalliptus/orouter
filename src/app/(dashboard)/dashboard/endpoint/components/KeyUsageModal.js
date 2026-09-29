"use client";

import { useEffect, useState } from "react";
import { Button, Card, Modal } from "@/shared/components";
import OverviewCards from "../../usage/components/OverviewCards";

const number = (value) => (value || 0).toLocaleString();
const date = (value) => value ? new Date(value).toLocaleString() : "None in this period";

export default function KeyUsageModal({ apiKey, onClose }) {
  const [period, setPeriod] = useState("24h");
  const [page, setPage] = useState(1);
  const [refresh, setRefresh] = useState(0);
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const reload = () => {
    setLoading(true);
    setData(null);
    setError("");
  };

  useEffect(() => {
    const controller = new AbortController();
    fetch(`/api/keys/${encodeURIComponent(apiKey.id)}/usage?period=${period}&page=${page}&pageSize=20`, {
      cache: "no-store", signal: controller.signal,
    }).then(async (response) => {
      if (!response.ok) throw new Error(response.status === 401 ? "Sign in to view key usage." : response.status === 404 ? "This key no longer exists." : "Unable to load usage. Try refreshing.");
      const result = await response.json();
      if (!controller.signal.aborted) setData(result);
    }).catch((err) => {
      if (!controller.signal.aborted) setError(err.message);
    }).finally(() => {
      if (!controller.signal.aborted) setLoading(false);
    });
    return () => controller.abort();
  }, [apiKey.id, period, page, refresh]);

  return (
    <Modal isOpen onClose={onClose} title={`Usage · ${apiKey.name}`} size="full">
      <div className="flex min-w-0 flex-col gap-5">
        <p className="text-xs text-text-muted break-all">Key ID: {apiKey.id}</p>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <label className="flex items-center gap-2 text-sm">
            Period
            <select value={period} onChange={(event) => { reload(); setPeriod(event.target.value); setPage(1); }} className="rounded-lg border border-border bg-surface px-3 py-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary">
              <option value="24h">Last 24 hours</option>
              <option value="7d">Last 7 days</option>
              <option value="30d">Last 30 days</option>
              <option value="60d">Last 60 days</option>
            </select>
          </label>
          <Button size="sm" variant="ghost" disabled={loading} onClick={() => { reload(); setRefresh((value) => value + 1); }}>Refresh</Button>
        </div>
        <p className="text-xs text-text-muted">Recorded requests for this API key only. Totals and history cover retained records in the selected period; older requests may have expired. Costs are estimates, not billing.</p>
        <div aria-busy={loading} className="flex min-w-0 flex-col gap-5">
          {loading && <p role="status" className="py-8 text-text-muted">Loading key usage…</p>}
          {error && <p role="alert" className="text-error">{error}</p>}
          {data && <>
            <OverviewCards stats={{ totalRequests: data.summary.requests, totalPromptTokens: data.summary.promptTokens, totalCompletionTokens: data.summary.completionTokens, totalCachedTokens: data.summary.cachedTokens, totalCost: data.summary.cost }} />
            <p className="text-sm text-text-muted">Last use in period: {date(data.summary.lastUsed)}</p>
            <UsageRecords title="Model breakdown" rows={data.models} />
            <UsageRecords title="Request history" rows={data.requests} history />
            <nav aria-label="Request history pages" className="flex flex-wrap items-center justify-between gap-3">
              <Button size="sm" variant="ghost" disabled={page <= 1} onClick={() => { reload(); setPage((value) => value - 1); }}>Previous</Button>
              <span role="status" className="text-xs text-text-muted">Page {page} of {Math.max(1, data.pagination.totalPages)} · {number(data.pagination.total)} requests</span>
              <Button size="sm" variant="ghost" disabled={page >= data.pagination.totalPages} onClick={() => { reload(); setPage((value) => value + 1); }}>Next</Button>
            </nav>
          </>}
        </div>
      </div>
    </Modal>
  );
}

function UsageRecords({ title, rows, history = false }) {
  return (
    <Card className="min-w-0 overflow-hidden">
      <h3 className="data-label mb-3">{title}</h3>
      {!rows.length ? <p className="py-4 text-sm text-text-muted">No requests in this period{history ? " on this page" : ""}.</p> : (
        <div className="overflow-x-auto focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary" tabIndex={0} role="region" aria-label={title}>
          <table className="w-full text-left text-xs whitespace-nowrap">
            <caption className="sr-only">{title}</caption>
            <thead><tr className="border-b border-border text-text-muted">
              {(history ? ["Time", "Model / Provider", "Status", "Input", "Cached", "Output", "Est. cost"] : ["Model / Provider", "Requests", "Last use", "Input", "Cached", "Output", "Est. cost"]).map((label) => <th key={label} scope="col" className="px-3 py-2">{label}</th>)}
            </tr></thead>
            <tbody className="divide-y divide-border font-mono">
              {rows.map((row) => <tr key={history ? row.id : JSON.stringify([row.model, row.provider])}>
                {history && <td className="px-3 py-2">{date(row.timestamp)}</td>}
                <th scope="row" className="px-3 py-2 font-normal">{row.model || "Unknown"}<span className="block text-text-muted">{row.provider || "Unknown"}</span></th>
                {history ? <td className="px-3 py-2">{row.status || "Unknown"}</td> : <><td className="px-3 py-2">{number(row.requests)}</td><td className="px-3 py-2">{date(row.lastUsed)}</td></>}
                <td className="px-3 py-2 text-info">{number(row.promptTokens)}</td>
                <td className="px-3 py-2">{number(row.cachedTokens)}</td>
                <td className="px-3 py-2 text-success">{number(row.completionTokens)}</td>
                <td className="px-3 py-2 text-warning">${(row.cost || 0).toFixed(6)}</td>
              </tr>)}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  );
}
