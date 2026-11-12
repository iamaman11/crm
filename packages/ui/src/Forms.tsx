import { useId, useEffect, useState } from "react";
import type { ComponentProps, ReactNode } from "react";
import styles from "./Forms.module.css";

export function TextField({label,...props}:ComponentProps<"input">&{label:string}) {
  const id=useId();
  return <label className={styles.field} htmlFor={id}><span>{label}</span><input {...props} id={id}/></label>;
}
export function SelectField({label,children,...props}:ComponentProps<"select">&{label:string}) {
  const id=useId();
  return <div className={styles.field}><label htmlFor={id}>{label}</label><select {...props} id={id}>{children}</select></div>;
}
export function FormGrid({children}: {children:ReactNode}) {
  return <div className={styles.grid}>{children}</div>;
}
export function Card({title,children}: {title:string;children:ReactNode}) {
  return <section className={styles.card}><h2>{title}</h2>{children}</section>;
}
export function ThemePicker() {
  const [theme,setTheme]=useState(()=>{
    const saved=localStorage.getItem("crm.theme");
    return saved==="light"||saved==="dark"?saved:"system";
  });
  useEffect(()=>{
    document.documentElement.dataset.theme=theme;
    localStorage.setItem("crm.theme",theme);
  },[theme]);
  return <SelectField label="Тема" value={theme} onChange={event=>setTheme(event.target.value)}>
    <option value="system">Системная</option><option value="light">Светлая</option><option value="dark">Тёмная</option>
  </SelectField>;
}
