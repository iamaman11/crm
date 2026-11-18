// Presentation-only decimal conversion. Rust supplies the authoritative catalog.
export type CurrencyCatalog=ReadonlyMap<string,number>;
export function currencyCatalog(entries:readonly {currencyCode:string;minorUnitExponent?:number|undefined}[]):CurrencyCatalog {
  const catalog=new Map<string,number>();
  for(const entry of entries) {
    const digits=entry.minorUnitExponent;
    if(!/^[A-Z]{3}$/.test(entry.currencyCode)||digits===undefined||!Number.isInteger(digits)||
        digits<0||digits>255||catalog.has(entry.currencyCode))
      throw Error("Некорректный справочник валют");
    catalog.set(entry.currencyCode,digits);
  }
  if(catalog.size===0)throw Error("Справочник валют не получен");
  return catalog;
}
function currencyDigits(currency:string,catalog:CurrencyCatalog):number {
  const digits=catalog.get(currency);
  if(digits===undefined)throw Error("Неизвестная точность валюты: "+currency);
  return digits;
}
export function minorUnits(value:string,currency:string,catalog:CurrencyCatalog):bigint {
  const match=/^(0|[1-9]\d*)(?:[.,](\d+))?$/.exec(value.trim());
  const whole=match?.[1];
  if(!whole)throw Error("Введите неотрицательную десятичную сумму");
  const digits=currencyDigits(currency,catalog);
  const fraction=match?.[2]??"";
  if(fraction.length>digits)throw Error("Для "+currency+" допустимо знаков после запятой: "+digits);
  const result=BigInt(whole)*10n**BigInt(digits)+BigInt(fraction.padEnd(digits,"0")||"0");
  if(result>9223372036854775807n)throw Error("Сумма превышает диапазон контракта");
  return result;
}
export function decimalUnits(value:bigint,currency:string,catalog:CurrencyCatalog):string {
  const digits=currencyDigits(currency,catalog),scale=10n**BigInt(digits);
  const n=value<0n?-value:value;
  return (value<0n?"-":"")+(n/scale).toString()+(digits?"."+(n%scale).toString().padStart(digits,"0"):"");
}
export function moneyText(value:{minorUnits:bigint;currencyCode:string}|undefined,catalog:CurrencyCatalog):string {
  return value?decimalUnits(value.minorUnits,value.currencyCode,catalog)+" "+value.currencyCode:"—";
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

// Exact weighted numerator divided by 10_000 basis points; never use JS
// Number for money, including sums larger than signed int64.
export function weightedMoneyText(numerator:string,currency:string,catalog:CurrencyCatalog):string {
  const digits=currencyDigits(currency,catalog)+4;
  const scale=10n**BigInt(digits);
  const raw=BigInt(numerator);
  if(raw<0n)throw Error("Отрицательная взвешенная сумма");
  return (raw/scale).toString()+"."+String(raw%scale).padStart(digits,"0")+" "+currency;
}
