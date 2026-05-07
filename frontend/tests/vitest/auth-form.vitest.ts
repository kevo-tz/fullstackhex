import { describe, expect, test, beforeEach } from "vitest";

function buildAuthFormHTML(mode: "login" | "register"): string {
  const isLogin = mode === "login";
  return `<!DOCTYPE html>
<html><body>
  <div class="card">
    <div class="error-msg" id="auth-error"></div>
    <form id="auth-form" method="post" data-is-login="${isLogin}">
      ${isLogin ? "" : `
      <div class="form-group">
        <label for="name">Name</label>
        <input id="name" name="name" type="text" placeholder="Your name (optional)" />
      </div>`}
      <div class="form-group">
        <label for="email">Email</label>
        <input id="email" name="email" type="email" placeholder="you@example.com" required />
      </div>
      <div class="form-group">
        <label for="password">Password</label>
        <input id="password" name="password" type="password" placeholder="Min 8 characters" required minlength="8" autocomplete="${isLogin ? "current-password" : "new-password"}" />
      </div>
      <button type="submit" class="btn" id="submit-btn">${isLogin ? "Sign in" : "Create account"}</button>
    </form>
    <div id="oauth-section" style="display:none">
      <div class="divider">or continue with</div>
      <div id="oauth-buttons"></div>
    </div>
  </div>
  <div class="form-footer">
    <a href="${isLogin ? "/register" : "/login"}">${isLogin ? "Need an account? Register" : "Already have an account? Sign in"}</a>
  </div>
</body></html>`;
}

/**
 * Mirrors the showError/hideError functions from AuthForm.astro inline script.
 */
function showError(msg: string): void {
  const el = document.getElementById("auth-error") as HTMLDivElement | null;
  if (!el) return;
  el.textContent = msg;
  el.classList.add("visible");
}
function hideError(): void {
  const el = document.getElementById("auth-error") as HTMLDivElement | null;
  if (!el) return;
  el.textContent = "";
  el.classList.remove("visible");
}

function renderOAuth(providers: string[]): void {
  const section = document.getElementById("oauth-section") as HTMLDivElement | null;
  const buttons = document.getElementById("oauth-buttons") as HTMLDivElement | null;
  if (!section || !buttons) return;
  if (providers.length === 0) {
    section.style.display = "none";
    return;
  }
  section.style.display = "block";
  buttons.innerHTML = providers
    .map(p => `<a href="/api/auth/oauth/${p}" class="btn btn-oauth">${p[0].toUpperCase() + p.slice(1)}</a>`)
    .join("");
}

describe("AuthForm", () => {
  describe("login mode", () => {
    beforeEach(() => {
      document.body.innerHTML = buildAuthFormHTML("login");
    });

    test("renders sign in title", () => {
      const btn = document.getElementById("submit-btn") as HTMLButtonElement | null;
      expect(btn).not.toBeNull();
      expect(btn!.textContent).toBe("Sign in");
    });

    test("does not render name field", () => {
      expect(document.getElementById("name")).toBeNull();
    });

    test("renders email and password fields", () => {
      expect(document.getElementById("email")).not.toBeNull();
      expect(document.getElementById("password")).not.toBeNull();
    });

    test("has password autocomplete set to current-password", () => {
      const pw = document.getElementById("password") as HTMLInputElement | null;
      expect(pw!.getAttribute("autocomplete")).toBe("current-password");
    });

    test("submit button enabled by default", () => {
      const btn = document.getElementById("submit-btn") as HTMLButtonElement | null;
      expect(btn!.disabled).toBe(false);
    });

    test("link points to register", () => {
      const link = document.querySelector(".form-footer a") as HTMLAnchorElement | null;
      expect(link).not.toBeNull();
      expect(link!.getAttribute("href")).toBe("/register");
    });

    test("link text shows register prompt", () => {
      const link = document.querySelector(".form-footer a") as HTMLAnchorElement | null;
      expect(link!.textContent).toBe("Need an account? Register");
    });

    test("password field has minlength 8", () => {
      const pw = document.getElementById("password") as HTMLInputElement | null;
      expect(pw!.getAttribute("minlength")).toBe("8");
    });

    test("email field is required", () => {
      const email = document.getElementById("email") as HTMLInputElement | null;
      expect(email!.required).toBe(true);
    });

    test("form data-is-login attribute is true", () => {
      const form = document.getElementById("auth-form") as HTMLFormElement | null;
      expect(form!.dataset.isLogin).toBe("true");
    });
  });

  describe("register mode", () => {
    beforeEach(() => {
      document.body.innerHTML = buildAuthFormHTML("register");
    });

    test("renders create account title", () => {
      const btn = document.getElementById("submit-btn") as HTMLButtonElement | null;
      expect(btn!.textContent).toBe("Create account");
    });

    test("renders name field", () => {
      expect(document.getElementById("name")).not.toBeNull();
    });

    test("renders email and password fields", () => {
      expect(document.getElementById("email")).not.toBeNull();
      expect(document.getElementById("password")).not.toBeNull();
    });

    test("has password autocomplete set to new-password", () => {
      const pw = document.getElementById("password") as HTMLInputElement | null;
      expect(pw!.getAttribute("autocomplete")).toBe("new-password");
    });

    test("link points to login", () => {
      const link = document.querySelector(".form-footer a") as HTMLAnchorElement | null;
      expect(link!.getAttribute("href")).toBe("/login");
    });

    test("link text shows sign in prompt", () => {
      const link = document.querySelector(".form-footer a") as HTMLAnchorElement | null;
      expect(link!.textContent).toBe("Already have an account? Sign in");
    });

    test("form data-is-login attribute is false", () => {
      const form = document.getElementById("auth-form") as HTMLFormElement | null;
      expect(form!.dataset.isLogin).toBe("false");
    });
  });

  describe("validation", () => {
    beforeEach(() => {
      document.body.innerHTML = buildAuthFormHTML("login");
    });

    test("email input type is email", () => {
      const email = document.getElementById("email") as HTMLInputElement | null;
      expect(email!.type).toBe("email");
    });

    test("password input type is password", () => {
      const pw = document.getElementById("password") as HTMLInputElement | null;
      expect(pw!.type).toBe("password");
    });

    test("fields have placeholder text", () => {
      const email = document.getElementById("email") as HTMLInputElement | null;
      const pw = document.getElementById("password") as HTMLInputElement | null;
      expect(email!.placeholder).toBeTruthy();
      expect(pw!.placeholder).toBeTruthy();
    });

    test("error element exists and is hidden by default", () => {
      const err = document.getElementById("auth-error") as HTMLDivElement | null;
      expect(err).not.toBeNull();
      expect(err!.classList.contains("visible")).toBe(false);
    });

    test("showError sets text and makes visible", () => {
      showError("Invalid credentials");
      const err = document.getElementById("auth-error") as HTMLDivElement | null;
      expect(err!.textContent).toBe("Invalid credentials");
      expect(err!.classList.contains("visible")).toBe(true);
    });

    test("hideError clears text and removes visibility", () => {
      showError("Invalid credentials");
      hideError();
      const err = document.getElementById("auth-error") as HTMLDivElement | null;
      expect(err!.textContent).toBe("");
      expect(err!.classList.contains("visible")).toBe(false);
    });

    test("showError and hideError are idempotent", () => {
      const err = document.getElementById("auth-error") as HTMLDivElement | null;
      showError("err");
      showError("err2");
      expect(err!.textContent).toBe("err2");
      hideError();
      hideError(); // second call — should not throw
      expect(err!.classList.contains("visible")).toBe(false);
    });
  });

  describe("OAuth provider rendering", () => {
    beforeEach(() => {
      document.body.innerHTML = buildAuthFormHTML("login");
    });

    test("oauth section hidden when no providers", () => {
      renderOAuth([]);
      const section = document.getElementById("oauth-section") as HTMLDivElement | null;
      expect(section!.style.display).toBe("none");
    });

    test("oauth section visible when providers exist", () => {
      renderOAuth(["github"]);
      const section = document.getElementById("oauth-section") as HTMLDivElement | null;
      expect(section!.style.display).toBe("block");
    });

    test("renders single provider button", () => {
      renderOAuth(["github"]);
      const buttons = document.getElementById("oauth-buttons") as HTMLDivElement | null;
      expect(buttons!.innerHTML).toContain("/api/auth/oauth/github");
      expect(buttons!.innerHTML).toContain("Github");
    });

    test("renders multiple provider buttons", () => {
      renderOAuth(["github", "google", "microsoft"]);
      const buttons = document.getElementById("oauth-buttons") as HTMLDivElement | null;
      expect(buttons!.innerHTML).toContain("/api/auth/oauth/github");
      expect(buttons!.innerHTML).toContain("/api/auth/oauth/google");
      expect(buttons!.innerHTML).toContain("/api/auth/oauth/microsoft");
      expect(buttons!.innerHTML).toContain("Github");
      expect(buttons!.innerHTML).toContain("Google");
      expect(buttons!.innerHTML).toContain("Microsoft");
    });

    test("renders no buttons when providers empty", () => {
      renderOAuth([]);
      const buttons = document.getElementById("oauth-buttons") as HTMLDivElement | null;
      expect(buttons!.innerHTML).toBe("");
    });

    test("capitalizes provider names correctly", () => {
      renderOAuth(["github"]);
      const buttons = document.getElementById("oauth-buttons") as HTMLDivElement | null;
      expect(buttons!.innerHTML).toContain(">Github</a>");
    });
  });
});
