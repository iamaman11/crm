/** Official Playwright newContext device emulation; NOT a page viewport resize. */
export const DEVICE_TYPES = ["desktop", "mobile", "tablet"] as const;
export type BrowserDevice = (typeof DEVICE_TYPES)[number];
export function chooseDevice(requested: BrowserDevice | undefined, active: BrowserDevice | null): BrowserDevice {
  if (requested !== undefined && !DEVICE_TYPES.includes(requested))
    throw new Error("BROWSER_DEVICE_INVALID");
  if (requested && active && requested !== active)
    throw new Error("BROWSER_DEVICE_CHANGE_REQUIRES_CLOSE");
  return requested ?? active ?? "desktop";
}
export function deviceOptions(device: BrowserDevice) {
  const common = { locale: "en-US", timezoneId: "UTC" };
  switch (device) {
    case "desktop": return {
      ...common, viewport: {width:1280,height:800}, screen:{width:1280,height:800},
      isMobile:false, hasTouch:false, deviceScaleFactor:1,
    };
    case "mobile": return {
      ...common, viewport:{width:390,height:844},screen:{width:390,height:844},
      isMobile:true,hasTouch:true,deviceScaleFactor:3,
      userAgent:"Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
    };
    case "tablet": return {
      ...common, viewport:{width:820,height:1180},screen:{width:820,height:1180},
      isMobile:true,hasTouch:true,deviceScaleFactor:2,
      userAgent:"Mozilla/5.0 (Linux; Android 14; Pixel Tablet) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    };
  }
}
