// The page the desktop shell loads: the window's board, which asks the Rust side for what to
// draw.

import { mount } from "svelte";
import Window from "./lib/Window.svelte";
import "./skins";
import "./styles/base.css";

mount(Window, { target: document.getElementById("app")! });
