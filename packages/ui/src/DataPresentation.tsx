import type {ComponentProps,ReactNode} from "react";

/** Native accessible presentation only. No domain types, transport or business rules. */
export function ScrollTable({label,children}:{label:string;children:ReactNode}) {
  return <div className="crm-table-scroll" role="region" aria-label={label} tabIndex={0}>{children}</div>;
}

export function FilterForm(props:Omit<ComponentProps<"form">,"className"|"style">) {
  return <form {...props} className="crm-filter-form"/>;
}

export function MetricGrid({children}:{children:ReactNode}) {
  return <dl className="crm-metric-grid">{children}</dl>;
}

type TrendRow={label:string;values:readonly [bigint|undefined,bigint|undefined];formatted:readonly [string,string]};

/** Pixel scaling only; callers supply authoritative values and their formatted labels. */
export function DivergingTrend({label,title,description,series,rows}:{label:string;title:string;description:string;series:readonly [string,string];rows:readonly TrendRow[]}) {
  const abs=(n:bigint)=>n<0n?-n:n;
  const max=rows.reduce((m,row)=>row.values.reduce<bigint>((a,n)=>n!==undefined&&abs(n)>a?abs(n):a,m),1n);
  const width=(n:bigint)=>String(abs(n)*5000n/max/100n)+"%";
  return <section aria-label={label} className="crm-trend">
    <h5>{title}</h5>
    <p>{description}</p>
    {rows.map(row=><div className="crm-trend-row" key={row.label}>
      <span>{row.label}</span>
      <div className="crm-trend-plot">
        {row.values.map((n,i)=><div key={series[i]} className="crm-trend-track" title={series[i]+": "+row.formatted[i]}>
          {n!==undefined&&<span className={i===0?"crm-trend-reference":"crm-trend-value"} style={{width:width(n),...(n<0n?{right:"50%"}:{left:"50%"})}}/>}
        </div>)}
      </div>
      <span>{row.formatted[1]}</span>
    </div>)}
  </section>;
}
