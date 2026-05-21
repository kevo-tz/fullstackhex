export function getCsrfToken(): string {
  return sessionStorage.getItem("csrf_token")
    || document.cookie.split("; ").find(r => r.startsWith("csrf_token="))?.split("=")[1]
    || "";
}
