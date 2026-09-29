// The skins a board can wear. Each is one stylesheet that fills the tokens base.css names;
// adding a skin adds a file and an entry here, and touches no component.

import "./blackboard.css";

export const skins = [{ id: "blackboard", name: "黑板" }] as const;

export type Skin = (typeof skins)[number]["id"];
