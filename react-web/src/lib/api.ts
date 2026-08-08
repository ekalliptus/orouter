// Ported + extended from src/shared/utils/api.js. Adds credentials:"include"
// (httpOnly JWT cookie auth) and a typed json() helper. The old app's stores
// called fetch() directly; those were rewritten to go through here in M3, but
// the bare get/post/put/del shape is preserved for drop-in call sites.

export class ApiError extends Error {
  status: number;
  data: unknown;
  constructor(message: string, status: number, data: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.data = data;
  }
}

const DEFAULT_HEADERS = { "Content-Type": "application/json" };

interface RequestOptions {
  headers?: Record<string, string>;
  signal?: AbortSignal;
}

async function handleResponse<T = unknown>(response: Response): Promise<T> {
  const data = (await response.json().catch(() => null)) as (T & { error?: string }) | null;
  if (!response.ok) {
    throw new ApiError(data?.error ?? "An error occurred", response.status, data);
  }
  return data as T;
}

export async function get<T = unknown>(url: string, options: RequestOptions = {}): Promise<T> {
  const response = await fetch(url, {
    method: "GET",
    credentials: "include",
    headers: { ...DEFAULT_HEADERS, ...options.headers },
    signal: options.signal,
  });
  return handleResponse<T>(response);
}

export async function post<T = unknown>(url: string, data?: unknown, options: RequestOptions = {}): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    credentials: "include",
    headers: { ...DEFAULT_HEADERS, ...options.headers },
    body: data === undefined ? undefined : JSON.stringify(data),
    signal: options.signal,
  });
  return handleResponse<T>(response);
}

export async function put<T = unknown>(url: string, data?: unknown, options: RequestOptions = {}): Promise<T> {
  const response = await fetch(url, {
    method: "PUT",
    credentials: "include",
    headers: { ...DEFAULT_HEADERS, ...options.headers },
    body: data === undefined ? undefined : JSON.stringify(data),
    signal: options.signal,
  });
  return handleResponse<T>(response);
}

export async function del<T = unknown>(url: string, options: RequestOptions = {}): Promise<T> {
  const response = await fetch(url, {
    method: "DELETE",
    credentials: "include",
    headers: { ...DEFAULT_HEADERS, ...options.headers },
    signal: options.signal,
  });
  return handleResponse<T>(response);
}

const api = { get, post, put, del };
export default api;
