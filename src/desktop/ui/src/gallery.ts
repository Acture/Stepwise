// The design review: every screen from fixtures, each in a window-sized frame. Nothing here
// talks to Rust; clicks are logged so the frames stay still.

import { mount } from "svelte";
import Gallery from "./lib/Gallery.svelte";
import "./skins";
import "./styles/base.css";

mount(Gallery, { target: document.getElementById("gallery")! });
