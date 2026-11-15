import type { BrowserContext } from "@cloudflare/playwright";
import { createHash } from "node:crypto";

// Browser screenshot-baseline stays only in the already-owned DO SQLite and
// gets deleted on explicit Close. Never store raw CRM screenshots in CI logs.
export const MAX_VISUAL_PNG_BYTES = 900_000;
export function pngReceipt(bytes: Buffer) {
  if (!bytes.length || bytes.length > MAX_VISUAL_PNG_BYTES)
    throw new Error("BROWSER_VISUAL_IMAGE_OUT_OF_RANGE");
  return { bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") };
}
export function visualThreshold(input: number | undefined): number {
  const value = input ?? 0.005;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 0.25)
    throw new Error("BROWSER_VISUAL_THRESHOLD_OUT_OF_RANGE");
  return value;
}
export type VisualComparison = {
  passed:boolean; threshold:number; differenceRatio:number;
  changedPixels:number|null; totalPixels:number|null;
  baselineDimensions:{width:number;height:number};
  currentDimensions:{width:number;height:number};
  dimensionsMatch:boolean;
};
export async function comparePngPixels(
  context: BrowserContext, before: string, after: string, threshold: number,
): Promise<VisualComparison> {
  // Use a fresh about:blank page in the SAME Playwright context. This avoids
  // trusting the tested site's patched Image/Canvas JS globals.
  const scratch = await context.newPage();
  try {
    return await scratch.evaluate(async ({before,after,threshold}) => {
      async function pixels(base64:string) {
        const data=atob(base64);
        const bytes=new Uint8Array(data.length);
        for(let i=0;i<data.length;i++) bytes[i]=data.charCodeAt(i);
        const image=await createImageBitmap(new Blob([bytes],{type:"image/png"}));
        try {
          if(image.width*image.height>3_000_000)
            throw new Error("BROWSER_VISUAL_PIXELS_LIMIT");
          const canvas=document.createElement("canvas");
          canvas.width=image.width;canvas.height=image.height;
          const ctx=canvas.getContext("2d",{willReadFrequently:true});
          if(!ctx) throw new Error("BROWSER_VISUAL_CANVAS_UNAVAILABLE");
          ctx.drawImage(image,0,0);
          return {width:image.width,height:image.height,
            data:ctx.getImageData(0,0,image.width,image.height).data};
        } finally {image.close();}
      }
      const a=await pixels(before),b=await pixels(after);
      const baselineDimensions={width:a.width,height:a.height};
      const currentDimensions={width:b.width,height:b.height};
      if(a.width!==b.width||a.height!==b.height) {
        return {passed:false,threshold,differenceRatio:1,
          changedPixels:null,totalPixels:null,baselineDimensions,currentDimensions,dimensionsMatch:false};
      }
      let changed=0;
      for(let i=0;i<a.data.length;i+=4){
        const delta=Math.max(Math.abs(a.data[i]!-b.data[i]!),
          Math.abs(a.data[i+1]!-b.data[i+1]!),
          Math.abs(a.data[i+2]!-b.data[i+2]!),
          Math.abs(a.data[i+3]!-b.data[i+3]!));
        if(delta>24) changed++;
      }
      const total=a.width*a.height;
      const ratio=total ? changed/total : 1;
      return {passed:ratio<=threshold,threshold,
        differenceRatio:Math.round(ratio*1_000_000)/1_000_000,
        changedPixels:changed,totalPixels:total,
        baselineDimensions,currentDimensions,dimensionsMatch:true};
    }, {before,after,threshold});
  } finally {
    try{await scratch.close();}catch{/*Provider session may have expired*/}
  }
}
