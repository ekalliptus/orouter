import { NextResponse } from "next/server";
import { getSettings } from "@/lib/localDb";
import { findPython310, getInstalledHeadroomExtras, HEADROOM_COMPRESSION_EXTRAS } from "@/lib/headroom/detect";
import { installHeadroomExtras, uninstallHeadroomExtras, getInstallLogTail } from "@/lib/headroom/process";
import { DEFAULT_HEADROOM_URL, isLoopbackHeadroomUrl } from "@/lib/headroom/detect";

export const dynamic = "force-dynamic";

// Pass the user's headroom settings (port + extra flags) into the installer so
// the automatic post-install restart matches how the proxy was running before.
async function restartOptions() {
  const settings = await getSettings();
  const url = settings.headroomUrl || DEFAULT_HEADROOM_URL;
  let port = 8787;
  try {
    const p = parseInt(new URL(url).port, 10);
    if (p > 0 && p < 65536) port = p;
  } catch { /* ignore, fall back to default */ }
  return {
    port,
    codeAware: settings.headroomCodeAware === true,
    kompress: settings.headroomKompress !== false,
    loopback: isLoopbackHeadroomUrl(url),
  };
}

export async function GET(req) {
  try {
    // `?log=1` returns the live install/uninstall log tail for progress polling.
    if (new URL(req.url).searchParams.get("log") === "1") {
      return NextResponse.json({ log: getInstallLogTail() });
    }
    const python = findPython310();
    const status = getInstalledHeadroomExtras(python);
    return NextResponse.json({
      available: HEADROOM_COMPRESSION_EXTRAS,
      ...status,
    });
  } catch (error) {
    return NextResponse.json({ error: error.message }, { status: 500 });
  }
}

export async function POST(req) {
  try {
    const body = await req.json().catch(() => ({}));
    const requested = Array.isArray(body?.extras) ? body.extras : [];
    const result = await installHeadroomExtras(requested, await restartOptions());
    return NextResponse.json(result);
  } catch (error) {
    const status = error.code === "NOT_INSTALLED" || error.code === "NO_PYTHON" ? 400 : 500;
    return NextResponse.json({ error: error.message, code: error.code || null }, { status });
  }
}

export async function DELETE(req) {
  try {
    const body = await req.json().catch(() => ({}));
    const requested = Array.isArray(body?.extras) ? body.extras : [];
    const result = await uninstallHeadroomExtras(requested);
    return NextResponse.json(result);
  } catch (error) {
    const status = error.code === "NO_PYTHON" || error.code === "INVALID_EXTRAS" ? 400 : 500;
    return NextResponse.json({ error: error.message, code: error.code || null }, { status });
  }
}
