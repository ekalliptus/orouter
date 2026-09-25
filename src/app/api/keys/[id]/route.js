import { NextResponse } from "next/server";
import { deleteApiKey, getApiKeyById, updateApiKey } from "@/lib/localDb";

// GET /api/keys/[id] - Get single key
export async function GET(request, { params }) {
  try {
    const { id } = await params;
    const key = await getApiKeyById(id);
    if (!key) {
      return NextResponse.json({ error: "Key not found" }, { status: 404 });
    }
    return NextResponse.json({ key });
  } catch (error) {
    console.log("Error fetching key:", error);
    return NextResponse.json({ error: "Failed to fetch key" }, { status: 500 });
  }
}

// PUT /api/keys/[id] - Update key
// Policy fields: isActive, maxDevices (0 = unlimited), allowedModels (array),
// boundDevices (array — lets the user unbind a device manually).
export async function PUT(request, { params }) {
  try {
    const { id } = await params;
    const body = await request.json();
    const { isActive, maxDevices, allowedModels, boundDevices, name, expiresAt } = body;

    const existing = await getApiKeyById(id);
    if (!existing) {
      return NextResponse.json({ error: "Key not found" }, { status: 404 });
    }

    const updateData = {};
    if (isActive !== undefined) updateData.isActive = isActive;
    if (name !== undefined) updateData.name = name;
    if (maxDevices !== undefined) {
      const n = Number(maxDevices);
      if (!Number.isFinite(n) || n < 0) {
        return NextResponse.json({ error: "maxDevices must be a number >= 0" }, { status: 400 });
      }
      updateData.maxDevices = Math.floor(n);
    }
    if (allowedModels !== undefined) {
      if (!Array.isArray(allowedModels) || allowedModels.some((m) => typeof m !== "string")) {
        return NextResponse.json({ error: "allowedModels must be an array of strings" }, { status: 400 });
      }
      updateData.allowedModels = allowedModels;
    }
    if (boundDevices !== undefined) {
      if (!Array.isArray(boundDevices) || boundDevices.some((d) => typeof d !== "string")) {
        return NextResponse.json({ error: "boundDevices must be an array of strings" }, { status: 400 });
      }
      updateData.boundDevices = boundDevices;
    }
    if (expiresAt !== undefined) {
      // null/empty clears expiry; otherwise must be a valid date.
      if (expiresAt === null || expiresAt === "") {
        updateData.expiresAt = null;
      } else if (Number.isNaN(new Date(expiresAt).getTime())) {
        return NextResponse.json({ error: "expiresAt must be a valid date" }, { status: 400 });
      } else {
        updateData.expiresAt = new Date(expiresAt).toISOString();
      }
    }

    const updated = await updateApiKey(id, updateData);

    return NextResponse.json({ key: updated });
  } catch (error) {
    console.log("Error updating key:", error);
    return NextResponse.json({ error: "Failed to update key" }, { status: 500 });
  }
}

// DELETE /api/keys/[id] - Delete API key
export async function DELETE(request, { params }) {
  try {
    const { id } = await params;

    const deleted = await deleteApiKey(id);
    if (!deleted) {
      return NextResponse.json({ error: "Key not found" }, { status: 404 });
    }

    return NextResponse.json({ message: "Key deleted successfully" });
  } catch (error) {
    console.log("Error deleting key:", error);
    return NextResponse.json({ error: "Failed to delete key" }, { status: 500 });
  }
}
