import {useId} from "react";
import type {ReactNode} from "react";
import styles from "./RecordCard.module.css";
export type RecordMode="collapsed"|"view"|"edit";
/** Controlled presentation only: the feature owns the record, active mode and commands. */
export function RecordCard({title,summary,mode,onToggle,actions,children}:{
  title:string;summary:ReactNode;mode:RecordMode;onToggle:()=>void;
  actions?:ReactNode;children:ReactNode;
}) {
  const panelId=useId();
  return <section className={styles.card}>
    <header className={styles.header}>
      <div className={styles.heading}><h2>{title}</h2><div className={styles.summary}>{summary}</div></div>
      <div className={styles.actions}>
        <button type="button" className={styles.toggle} aria-expanded={mode!=="collapsed"}
          aria-controls={panelId} onClick={onToggle}>{mode==="collapsed"?"Открыть карточку":"Свернуть"}</button>
        {actions}
      </div>
    </header>
    <div id={panelId} role="region" aria-label={title} hidden={mode==="collapsed"} className={styles.content}>
      {mode==="collapsed"?null:children}
    </div>
  </section>;
}
