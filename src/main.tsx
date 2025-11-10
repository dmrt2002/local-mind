import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";
import "./index.css";
import { logger } from "./utils/logger";

logger.info("main.tsx: Application starting");

// Wait for DOM to be ready
function init() {
  logger.debug("main.tsx: init() called", { readyState: document.readyState });

  const rootElement = document.getElementById("root");

  if (!rootElement) {
    logger.error("main.tsx: Root element not found!");
    // Create it if it doesn't exist
    const newRoot = document.createElement("div");
    newRoot.id = "root";
    document.body.appendChild(newRoot);
    logger.info("main.tsx: Created root element");
    renderApp(newRoot);
  } else {
    logger.debug("main.tsx: Root element found");
    renderApp(rootElement);
  }
}

function renderApp(rootElement: HTMLElement) {
  try {
    logger.debug("main.tsx: Creating React root");
    const root = ReactDOM.createRoot(rootElement);
    
    logger.info("main.tsx: Rendering application with ErrorBoundary");
    root.render(
      <React.StrictMode>
        <ErrorBoundary>
          <App />
        </ErrorBoundary>
      </React.StrictMode>
    );
    
    logger.info("main.tsx: Application rendered successfully");
    
    // Verify render happened after a short delay
    setTimeout(() => {
      if (rootElement.textContent?.includes("Loading LocalMind")) {
        logger.warn("main.tsx: React content may not have updated");
      } else {
        logger.debug("main.tsx: Content updated successfully");
      }
    }, 500);
  } catch (error) {
    logger.error("main.tsx: Render error", error);
    const errorMsg = error instanceof Error ? error.message : String(error);
    const errorStack = error instanceof Error ? error.stack : "No stack trace";
    rootElement.innerHTML = `
      <div style="padding: 50px; color: red; font-size: 20px; background: white;">
        <h1>React Render Error</h1>
        <pre>${errorMsg}</pre>
        <pre style="font-size: 12px; overflow: auto;">${errorStack}</pre>
      </div>
    `;
  }
}

// Check if DOM is already loaded
if (document.readyState === "loading") {
  logger.debug("main.tsx: DOM loading, waiting for DOMContentLoaded");
  document.addEventListener("DOMContentLoaded", init);
} else {
  logger.debug("main.tsx: DOM ready, initializing");
  setTimeout(init, 0);
}
