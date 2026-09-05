import { mount } from "svelte";

import UiAblation from "./UiAblation.svelte";
import "./ui-ablation.css";

const target = document.querySelector<HTMLElement>("#app");
if (!target) throw new Error("missing-ui-ablation-root");

mount(UiAblation, { target });
