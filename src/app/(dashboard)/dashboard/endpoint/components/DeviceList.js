"use client";

// Bound-device chips inside the key policy editor. Copy feedback reuses the
// page-level useCopyToClipboard state ("dev-" + deviceId as the tag).
export default function DeviceList({ devices, maxDevices, copied, onCopy, onRemove, onUnbindAll }) {
  const list = devices || [];
  const max = Number(maxDevices) > 0 ? Number(maxDevices) : null;

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-text-muted">
          {`Bound Devices (${list.length}${max ? ` / ${max}` : ""})`}
        </span>
        {list.length > 0 && (
          <button
            type="button"
            onClick={onUnbindAll}
            className="text-xs text-red-500 hover:underline"
          >
            Unbind all
          </button>
        )}
      </div>
      {list.length === 0 ? (
        <p className="text-xs text-text-muted">
          No devices bound — the first device that uses this key claims a slot automatically.
        </p>
      ) : (
        <div className="flex flex-col gap-1">
          {list.map((d) => (
            <div key={d} className="flex items-center gap-2 rounded border border-border-subtle bg-surface-2 px-2 py-1">
              <span className="material-symbols-outlined text-[14px] text-text-muted">computer</span>
              <code className="flex-1 truncate font-mono text-xs" title={d}>{d}</code>
              <button
                type="button"
                onClick={() => onCopy(d)}
                title="Copy device ID"
                className="p-1 rounded hover:bg-surface-3 text-text-muted hover:text-primary transition-colors"
              >
                <span className="material-symbols-outlined text-[14px]">{copied === "dev-" + d ? "check" : "content_copy"}</span>
              </button>
              <button
                type="button"
                onClick={() => onRemove(d)}
                title="Unbind this device"
                className="p-1 rounded hover:bg-red-500/10 text-red-500 transition-colors"
              >
                <span className="material-symbols-outlined text-[14px]">link_off</span>
              </button>
            </div>
          ))}
        </div>
      )}
      {max && list.length >= max && (
        <p className="text-xs text-orange-500">
          All {max} slot(s) used — unbind a device or raise Max Devices, or new devices will be rejected.
        </p>
      )}
    </div>
  );
}
