import {useState} from "react";
import {Button,TextField} from "@ultimate-crm/ui";

export function SearchField({label,onSearch}:{label:string;onSearch:(value:string)=>void}) {
  const [value,setValue]=useState("");
  return <div>
    <TextField type="search" label={label} value={value} maxLength={200}
      placeholder="Фрагмент имени или ID"
      onChange={event=>setValue(event.target.value)}
      onKeyDown={event=>{if(event.key==="Enter"){
        event.preventDefault();event.stopPropagation();onSearch(value);
      }}}/>
    <small>Поиск учитывает регистр букв.</small>{" "}
    <Button variant="secondary" onClick={()=>onSearch(value)}>Найти</Button>{" "}
    <Button variant="secondary" onClick={()=>{setValue("");onSearch("");}}>Сбросить поиск</Button>
  </div>;
}
