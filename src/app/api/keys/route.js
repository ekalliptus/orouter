import { NextResponse } from "next/server";
import { getApiKeys, createApiKey, updateApiKey } from "@/lib/localDb";
import { getConsistentMachineId } from "@/shared/utils/machineId";
import { parsePolicyInput } from "./policyInput.js";

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

// POST /api/keys - Create new API key. Policy fields (maxDevices, allowedModels,
// expiresAt, tokenLimit) are optional and validated by parsePolicyInput.
export async function POST(request) {
  try {
    const body = await request.json();
    if (!body.name) {
      return NextResponse.json({ error: "Name is required" }, { status: 400 });
    }

    const { patch, error } = parsePolicyInput(body);
    if (error) {
      return NextResponse.json({ error }, { status: 400 });
    }

    // Always get machineId from server
    const machineId = await getConsistentMachineId();
    const apiKey = await createApiKey(body.name, machineId);

    if (Object.keys(patch).length > 0) {
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
      expiresAt: apiKey.expiresAt ?? null,
      tokenLimit: apiKey.tokenLimit ?? 0,
    }, { status: 201 });
  } catch (error) {
    console.log("Error creating key:", error);
    return NextResponse.json({ error: "Failed to create key" }, { status: 500 });
  }
}
