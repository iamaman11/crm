// Browser transport only: the server owns the exact canonical bytes.
export function downloadCsv(csv:Uint8Array,filename:string,mediaType="text/csv;charset=utf-8") {
  const url=URL.createObjectURL(new Blob([Uint8Array.from(csv)],{type:mediaType}));
  try {
    const link=document.createElement("a");
    link.href=url;link.download=filename;link.click();
  } finally {window.setTimeout(()=>URL.revokeObjectURL(url),0);}
}
