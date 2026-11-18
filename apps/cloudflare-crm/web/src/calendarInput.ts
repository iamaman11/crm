// Browser input/output adapter only. CRM's Rust CalendarDate owns civil validity.
export type Calendar={year:number;month:number;day:number};
export function inputDate(date:Calendar|undefined):string {
  return date?String(date.year).padStart(4,"0")+"-"+String(date.month).padStart(2,"0")+
    "-"+String(date.day).padStart(2,"0"):"";
}
export function parseDate(value:string):Calendar|undefined {
  if(!value)return undefined;
  if(!/^\d{4}-\d{2}-\d{2}$/.test(value))throw Error("Неверная дата закрытия");
  const [year,month,day]=value.split("-").map(Number);
  if(year===undefined||month===undefined||day===undefined)
    throw Error("Неверная дата закрытия");
  // Do not invent an instant or duplicate leap/day/month validity in React.
  // Even a well-shaped invalid civil date is rejected by the Rust owner.
  return {year,month,day};
}
