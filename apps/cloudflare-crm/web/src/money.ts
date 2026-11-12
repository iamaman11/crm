// Decimal input and display only; authoritative validation stays in Rust.
export function currencyDigits(currency:string):number {
  const digits=new Intl.NumberFormat("en",{style:"currency",currency}).resolvedOptions().maximumFractionDigits;
  if(digits===undefined)throw Error("Неизвестная точность валюты");
  return digits;
}
export function minorUnits(value:string,currency:string):bigint {
  const match=/^(0|[1-9]\d*)(?:[.,](\d+))?$/.exec(value.trim());
  const whole=match?.[1];
  if(!whole)throw Error("Введите неотрицательную десятичную сумму");
  const digits=currencyDigits(currency);
  const fraction=match?.[2]??"";
  if(fraction.length>digits)throw Error("Для "+currency+" допустимо знаков после запятой: "+digits);
  const result=BigInt(whole)*10n**BigInt(digits)+BigInt(fraction.padEnd(digits,"0")||"0");
  if(result>9223372036854775807n)throw Error("Сумма превышает диапазон контракта");
  return result;
}
export function decimalUnits(value:bigint,currency:string):string {
  const digits=currencyDigits(currency),scale=10n**BigInt(digits);
  const n=value<0n?-value:value;
  return (value<0n?"-":"")+(n/scale).toString()+(digits?"."+(n%scale).toString().padStart(digits,"0"):"");
}
export function moneyText(value:{minorUnits:bigint;currencyCode:string}|undefined):string {
  return value?decimalUnits(value.minorUnits,value.currencyCode)+" "+value.currencyCode:"—";
}
export function basisPoints(value:string):number {
  const match=/^(0|[1-9]\d*)(?:[.,](\d{1,2}))?$/.exec(value.trim());
  const whole=match?.[1];
  if(!whole)throw Error("Введите вероятность в процентах");
  const result=BigInt(whole)*100n+BigInt((match?.[2]??"").padEnd(2,"0")||"0");
  if(result>10000n)throw Error("Вероятность должна быть от 0 до 100%");
  return Number(result);
}
export function percent(value:number):string {
  return Math.trunc(value/100)+"."+String(value%100).padStart(2,"0");
}
