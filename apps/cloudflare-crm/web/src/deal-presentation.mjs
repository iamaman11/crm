// UI-only presentation for USD/EUR/PLN, all with 2 decimal places.
// crm-sales Rust remains the sole owner of money/probability business rules.
// Never use fractional JavaScript numbers for monetary conversion.
const DECIMAL=/^(0|[1-9]\d*)(?:[.,](\d{1,2}))?$/;

export function amountToMinorUnits(value){
  const match=DECIMAL.exec(String(value).trim());
  if(!match)throw Error("Введите сумму, например 127,50 (не более двух знаков после запятой)");
  const cents=BigInt(match[1])*100n+BigInt((match[2]||"").padEnd(2,"0")||"0");
  if(cents>BigInt(Number.MAX_SAFE_INTEGER))
    throw Error("Сумма слишком велика для безопасного ввода");
  return Number(cents);
}

export function amountFromMinorUnits(value){
  const units=BigInt(value);
  const absolute=units<0n?-units:units;
  return (units<0n?"-":"")+(absolute/100n).toString()+"."+
    (absolute%100n).toString().padStart(2,"0");
}

export function formatDealMoney(amount){
  if(!amount)return "—";
  const decimal=amountFromMinorUnits(amount.minor_units);
  const negative=decimal.startsWith("-");
  const [whole,fraction]=(negative?decimal.slice(1):decimal).split(".");
  const grouped=whole.replace(/\B(?=(\d{3})+(?!\d))/g,"\u00a0");
  return (negative?"-":"")+grouped+","+fraction+" "+amount.currency;
}

export function percentToBasisPoints(value){
  const match=DECIMAL.exec(String(value).trim());
  if(!match)throw Error("Введите вероятность от 0 до 100%, например 37,5");
  const whole=Number(match[1]);
  if(whole>100)throw Error("Вероятность должна быть от 0 до 100%");
  const bps=whole*100+Number((match[2]||"").padEnd(2,"0")||"0");
  if(bps>10000)throw Error("Вероятность должна быть от 0 до 100%");
  return bps;
}

export function percentFromBasisPoints(value){
  const bps=Number(value);
  if(!Number.isInteger(bps)||bps<0||bps>10000)return "—";
  const fraction=String(bps%100).padStart(2,"0").replace(/0+$/,"");
  return Math.floor(bps/100)+(fraction?"."+fraction:"");
}
