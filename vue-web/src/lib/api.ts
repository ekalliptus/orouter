// Typed fetch helpers against the Rust backend. credentials:"include" carries
// the httpOnly auth_token JWT cookie, mirroring the react-web api module.
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

async function handleResponse<T>(response: Response): Promise<T> {
  const data = (await response.json().catch(() => null)) as (T & { error?: string }) | null;
  if (!response.ok) {
    throw new ApiError(data?.error ?? "An error occurred", response.status, data);
  }
  return data as T;
}

async function request<T>(method: string, url: string, body?: unknown): Promise<T> {
  const response = await fetch(url, {
    method,
    credentials: "include",
    headers: DEFAULT_HEADERS,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return handleResponse<T>(response);
}

export const api = {
  get: <T = unknown>(url: string) => request<T>("GET", url),
  post: <T = unknown>(url: string, data?: unknown) => request<T>("POST", url, data),
  put: <T = unknown>(url: string, data?: unknown) => request<T>("PUT", url, data),
  patch: <T = unknown>(url: string, data?: unknown) => request<T>("PATCH", url, data),
  del: <T = unknown>(url: string) => request<T>("DELETE", url),
};
