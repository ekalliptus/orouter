import { NextResponse } from "next/server";
import { getApiKeys, createApiKey, updateApiKey } from "@/lib/localDb";
import { getConsistentMachineId } from "@/shared/utils/machineId";

export const dynamic = "force-dynamic";

// GET /api/keys - List API keys
export async function GET() {
  try {
    const keys = await getApiKeys();
    return NextResponse.json({ keys });
  } catch (error) {
    console.log("Error fetching keys:", error);
    return NextResponse.json({ error: "Failed to fetch keys" }, { status: 500 });
  }
}

// POST /api/keys - Create new API key
// Optional policy fields:
//   maxDevices    — number, 0 = unlimited
//   allowedModels — array of model ids; empty = all models
export async function POST(request) {
  try {
    const body = await request.json();
    const { name, maxDevices, allowedModels } = body;

    if (!name) {
      return NextResponse.json({ error: "Name is required" }, { status: 400 });
    }

    // Always get machineId from server
    const machineId = await getConsistentMachineId();
    const apiKey = await createApiKey(name, machineId);

    if (Array.isArray(allowedModels) || Number.isFinite(Number(maxDevices))) {
      const patch = {};
      if (Number.isFinite(Number(maxDevices))) patch.maxDevices = Math.max(0, Math.floor(Number(maxDevices)));
      if (Array.isArray(allowedModels)) patch.allowedModels = allowedModels.filter((m) => typeof m === "string");
      await updateApiKey(apiKey.id, patch);
      Object.assign(apiKey, patch);
    }

    return NextResponse.json({
      key: apiKey.key,
      name: apiKey.name,
      id: apiKey.id,
      machineId: apiKey.machineId,
      maxDevices: apiKey.maxDevices,
      allowedModels: apiKey.allowedModels,
    }, { status: 201 });
  } catch (error) {
    console.log("Error creating key:", error);
    return NextResponse.json({ error: "Failed to create key" }, { status: 500 });
  }
}
