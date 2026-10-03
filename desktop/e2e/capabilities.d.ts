// The one capability tauri-driver reads: which program to launch, with which arguments.
declare global {
	namespace WebdriverIO {
		interface Capabilities {
			"tauri:options"?: {
				application: string;
				args?: string[];
			};
		}
	}
}

export {};
