import {SelectField,TextField} from "@ultimate-crm/ui";
export function TimeFields({name,label,editing}:{name:string;label:string;editing:boolean}) {
  return <div>{editing&&<SelectField label={label+": изменение"} name={name+"Mode"} defaultValue="keep">
    <option value="keep">Сохранить</option><option value="set">Установить</option><option value="clear">Очистить</option>
  </SelectField>}<TextField label={label} name={name} type="datetime-local" step="1"/></div>;
}
