import "./TabNavigation.css";

export type TabType = "home" | "search" | "analytics" | "settings";

interface TabNavigationProps {
  activeTab: TabType;
  onTabChange: (tab: TabType) => void;
}

export default function TabNavigation({ activeTab, onTabChange }: TabNavigationProps) {
  return (
    <div className="tab-navigation">
      <button
        className={`tab-button ${activeTab === "home" ? "active" : ""}`}
        onClick={() => onTabChange("home")}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M2 6L8 2L14 6V13C14 13.5304 13.7893 14.0391 13.4142 14.4142C13.0391 14.7893 12.5304 15 12 15H4C3.46957 15 2.96086 14.7893 2.58579 14.4142C2.21071 14.0391 2 13.5304 2 13V6Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M6 15V8H10V15"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        Home
      </button>
      <button
        className={`tab-button ${activeTab === "search" ? "active" : ""}`}
        onClick={() => onTabChange("search")}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M7 12C9.76142 12 12 9.76142 12 7C12 4.23858 9.76142 2 7 2C4.23858 2 2 4.23858 2 7C2 9.76142 4.23858 12 7 12Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M10.5 10.5L14 14"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        Search
      </button>
      <button
        className={`tab-button ${activeTab === "analytics" ? "active" : ""}`}
        onClick={() => onTabChange("analytics")}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M2 14V10H6V14H2Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M10 14V2H14V14H10Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M2 6H6V2L2 6Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        Analytics
      </button>
      <button
        className={`tab-button ${activeTab === "settings" ? "active" : ""}`}
        onClick={() => onTabChange("settings")}
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M8 10C9.10457 10 10 9.10457 10 8C10 6.89543 9.10457 6 8 6C6.89543 6 6 6.89543 6 8C6 9.10457 6.89543 10 8 10Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M13 8C13 8.34 12.98 8.67 12.94 9L14.31 10.03C14.43 10.12 14.46 10.29 14.39 10.42L13.07 12.58C13 12.71 12.84 12.76 12.7 12.71L11.07 12.05C10.74 12.29 10.38 12.49 10 12.64L9.75 14.37C9.73 14.51 9.61 14.62 9.47 14.62H6.73C6.59 14.62 6.47 14.51 6.45 14.37L6.2 12.64C5.82 12.49 5.46 12.29 5.13 12.05L3.5 12.71C3.36 12.76 3.2 12.71 3.13 12.58L1.81 10.42C1.74 10.29 1.77 10.12 1.89 10.03L3.26 9C3.22 8.67 3.2 8.34 3.2 8C3.2 7.66 3.22 7.33 3.26 7L1.89 5.97C1.77 5.88 1.74 5.71 1.81 5.58L3.13 3.42C3.2 3.29 3.36 3.24 3.5 3.29L5.13 3.95C5.46 3.71 5.82 3.51 6.2 3.36L6.45 1.63C6.47 1.49 6.59 1.38 6.73 1.38H9.47C9.61 1.38 9.73 1.49 9.75 1.63L10 3.36C10.38 3.51 10.74 3.71 11.07 3.95L12.7 3.29C12.84 3.24 13 3.29 13.07 3.42L14.39 5.58C14.46 5.71 14.43 5.88 14.31 5.97L12.94 7C12.98 7.33 13 7.66 13 8Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        Settings
      </button>
    </div>
  );
}
