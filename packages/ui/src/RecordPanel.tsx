import {useEffect,useId,useRef} from "react";
import type {ReactNode} from "react";
import styles from "./RecordPanel.module.css";

/** The existing React open prop owns the panel; native dialog is only its projection. */
type PanelCloseSource="button"|"escape";
export function RecordPanel({open,title,onClose,dirty,children}:{
  open:boolean;title:string;onClose:()=>void;dirty?:boolean;children:ReactNode;
}) {
  const dialog=useRef<HTMLDialogElement>(null);
  const heading=useId();
  useEffect(()=>{
    const element=dialog.current;
    if(!element)return;
    if(open&&!element.open) {
      delete element.dataset.crmPanelLastClose;
      element.showModal();
    }
    if(!open&&element.open)element.close();
  },[open]);
  const requestClose=(source:PanelCloseSource)=>{
    if(dialog.current)dialog.current.dataset.crmPanelLastClose=source;
    onClose();
  };
  return <dialog ref={dialog} className={styles.panel} aria-labelledby={heading}
    data-crm-panel-desired={open?"open":"closed"}
    data-crm-panel-dirty={dirty===undefined?undefined:dirty?"true":"false"}
    onCancel={event=>{
      // Escape would otherwise close the native dialog before a dirty-form
      // guard can reject the request, leaving React open=true but dialog.open=false.
      event.preventDefault();
      requestClose("escape");
    }}>
    <header className={styles.header}>
      <h2 id={heading}>{title}</h2>
      <button type="button" className={styles.close} onClick={()=>requestClose("button")}
        aria-label="Закрыть панель" title="Закрыть панель">×</button>
    </header>
    <div className={styles.content}>{open?children:null}</div>
  </dialog>;
}

/** Shared visual layout for CRM page-level actions; business decisions stay in Rust. */
export function RecordActions({children}:{children:ReactNode}) {
  return <div className={styles.actions}>{children}</div>;
}
