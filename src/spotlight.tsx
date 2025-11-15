// Immediate log to verify script is loading
console.log("[Spotlight] Script module loading...");

import React from "react";
import ReactDOM from "react-dom/client";
import SpotlightSearch from "./components/SpotlightSearch";
import ErrorBoundary from "./components/ErrorBoundary";
// Don't import index.css - Spotlight should have isolated styles only

console.log("[Spotlight] Imports successful");

// Wrap initialization in try-catch for error handling
try {

  // Wait for DOM to be ready
  function init() {
    console.log("[Spotlight] Initializing...");
    const rootElement = document.getElementById("root");

    if (!rootElement) {
      console.warn("[Spotlight] Root element not found, creating new one");
      const newRoot = document.createElement("div");
      newRoot.id = "root";
      document.body.appendChild(newRoot);
      renderSpotlight(newRoot);
    } else {
      console.log("[Spotlight] Root element found, rendering");
      renderSpotlight(rootElement);
    }
  }

  function renderTest(rootElement: HTMLElement) {
    try {
      console.log("[Spotlight] Testing React render with simple div...");
      const root = ReactDOM.createRoot(rootElement);
      root.render(
        <div style={{ padding: "20px", color: "white", background: "rgba(0, 0, 0, 0.8)", borderRadius: "8px" }}>
          <h2>React is working! Loading Spotlight...</h2>
        </div>
      );
      console.log("[Spotlight] Test render successful");
      return root;
    } catch (error) {
      console.error("[Spotlight] Test render failed:", error);
      throw error;
    }
  }

  function renderSpotlight(rootElement: HTMLElement) {
    try {
      console.log("[Spotlight] Creating React root...");
      const root = ReactDOM.createRoot(rootElement);
      
      // First render a test to verify React works
      console.log("[Spotlight] Rendering test component first...");
      root.render(
        <div style={{ padding: "20px", color: "white", background: "rgba(0, 0, 0, 0.8)", borderRadius: "8px" }}>
          <h2>React is working! Loading Spotlight...</h2>
        </div>
      );
      
      // Wait a moment, then render the full component
      setTimeout(() => {
        try {
          console.log("[Spotlight] Rendering SpotlightSearch component...");
          root.render(
            <React.StrictMode>
              <ErrorBoundary>
                <SpotlightSearch />
              </ErrorBoundary>
            </React.StrictMode>
          );
          console.log("[Spotlight] Render complete");
        } catch (error) {
          console.error("[Spotlight] Component render error:", error);
          const errorMsg = error instanceof Error ? error.message : String(error);
          const stack = error instanceof Error ? error.stack : "";
          rootElement.innerHTML = `
            <div style="padding: 50px; color: red; font-size: 20px; background: rgba(255, 255, 255, 0.95); border-radius: 12px; margin: 50px;">
              <h1>Spotlight Component Error</h1>
              <pre style="background: #f5f5f5; padding: 15px; border-radius: 4px; overflow: auto;">${errorMsg}${stack ? '\n\n' + stack : ''}</pre>
            </div>
          `;
        }
      }, 100);
    } catch (error) {
      console.error("[Spotlight] Render error:", error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      const stack = error instanceof Error ? error.stack : "";
      rootElement.innerHTML = `
        <div style="padding: 50px; color: red; font-size: 20px; background: rgba(255, 255, 255, 0.95); border-radius: 12px; margin: 50px; position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 99999;">
          <h1>Spotlight Render Error</h1>
          <pre style="background: #f5f5f5; padding: 15px; border-radius: 4px; overflow: auto; max-height: 400px;">${errorMsg}${stack ? '\n\n' + stack : ''}</pre>
        </div>
      `;
    }
  }

  // Check if DOM is already loaded
  if (document.readyState === "loading") {
    console.log("[Spotlight] DOM loading, waiting for DOMContentLoaded");
    document.addEventListener("DOMContentLoaded", init);
  } else {
    console.log("[Spotlight] DOM ready, initializing");
    setTimeout(init, 0);
  }
} catch (error) {
  console.error("[Spotlight] Module-level error:", error);
  const rootElement = document.getElementById("root");
  if (rootElement) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? error.stack : "";
    rootElement.innerHTML = `
      <div style="padding: 50px; color: red; font-size: 20px; background: rgba(255, 255, 255, 0.95); border-radius: 12px; margin: 50px; position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); z-index: 99999;">
        <h1>Spotlight Module Error</h1>
        <p>Failed to load spotlight module. Check console for details.</p>
        <pre style="background: #f5f5f5; padding: 15px; border-radius: 4px; overflow: auto; max-height: 400px;">${errorMsg}${stack ? '\n\n' + stack : ''}</pre>
      </div>
    `;
  }
}

