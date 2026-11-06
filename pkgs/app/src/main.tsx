import { Provider } from "jotai";
import { createRoot } from "react-dom/client";
import "./view/styles.css";
import { App } from "./view/App.tsx";

createRoot(document.getElementById("root")!).render(
  <Provider>
    <App />
  </Provider>,
);
