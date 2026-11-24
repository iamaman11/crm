import {useEffect,useState} from "react";
import {Button,TextField} from "@ultimate-crm/ui";

export function SearchField({label,onSearch,submitted="",searchLabel="Найти",resetLabel="Сбросить поиск"}:{
  label:string;onSearch:(value:string)=>void;submitted?:string;
  searchLabel?:string;resetLabel?:string;
}) {
  const [value,setValue]=useState(submitted);
  useEffect(()=>setValue(submitted),[submitted]);
  return <div>
    <TextField type="search" label={label} value={value} maxLength={200}
      placeholder="Фрагмент имени или ID"
      onChange={event=>setValue(event.target.value)}
      onKeyDown={event=>{if(event.key==="Enter"){
        event.preventDefault();event.stopPropagation();onSearch(value);
      }}}/>
    <small>Поиск учитывает регистр букв.</small>{" "}
    <Button variant="secondary" onClick={()=>onSearch(value)}>{searchLabel}</Button>{" "}
    <Button variant="secondary" onClick={()=>{setValue("");onSearch("");}}>{resetLabel}</Button>
  </div>;
}
