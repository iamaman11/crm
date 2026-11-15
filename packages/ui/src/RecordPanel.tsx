import {useEffect,useId,useRef} from "react";
import type {ReactNode} from "react";
import styles from "./RecordPanel.module.css";

/** Shared presentation only. The Rust/Connect owners decide what records and actions mean. */
export function RecordPanel({open,title,onClose,children}:{
  open:boolean;title:string;onClose:()=>void;children:ReactNode;
}) {
  const dialog=useRef<HTMLDialogElement>(null);
  const heading=useId();
  useEffect(()=>{
    const element=dialog.current;
    if(!element)return;
    if(open&&!element.open)element.showModal();
    if(!open&&element.open)element.close();
  },[open]);
  return <dialog ref={dialog} className={styles.panel} aria-labelledby={heading}
    onClose={onClose}>
    <header className={styles.header}>
      <h2 id={heading}>{title}</h2>
      <button type="button" className={styles.close} onClick={onClose}
        aria-label="Закрыть панель" title="Закрыть панель">×</button>
    </header>
    <div className={styles.content}>{open?children:null}</div>
  </dialog>;
}

/** Shared visual layout for CRM page-level actions; business decisions stay in Rust. */
export function RecordActions({children}:{children:ReactNode}) {
  return <div className={styles.actions}>{children}</div>;
}
