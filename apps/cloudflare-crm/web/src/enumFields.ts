import {ContactPointKind} from "@ultimate-crm/client/cloudflare";

/** Presentation-only label for the canonical generated ContactPointKind enum. */
export function contactKindLabel(kind:ContactPointKind):string {
  switch(kind) {
    case ContactPointKind.EMAIL:return "Email";
    case ContactPointKind.PHONE:return "Телефон";
    case ContactPointKind.POSTAL:return "Почтовый адрес";
    case ContactPointKind.WEB:return "Сайт";
    case ContactPointKind.MESSAGING:return "Мессенджер";
    default:return "Неизвестный способ связи";
  }
}

/** The HTML boundary accepts only explicitly supported members of generated Protobuf enums. */
export function enumValue<T extends number>(raw:string,values:readonly T[],name:string):T {
  const found=values.find(value=>String(value)===raw);
  if(found===undefined)throw Error("Недопустимое значение: "+name);
  return found;
}
export function boundedInt(raw:string,min:number,max:number,name:string):number {
  const number=Number(raw);
  if(!/^\d+$/.test(raw)||!Number.isSafeInteger(number)||number<min||number>max)throw Error("Некорректное число: "+name);
  return number;
}
