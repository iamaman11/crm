import React from "react";
import {createRoot} from "react-dom/client";

function ContractOnly() {
  return (
    <main style={{maxWidth:"44rem",margin:"8vh auto",padding:"2rem",
      fontFamily:"system-ui, sans-serif",lineHeight:1.6,color:"CanvasText"}}>
      <h1>CRM: переход на Connect</h1>
      <p>Старые бизнес-подключения отключены. Система переходит на
      единые типизированные Protobuf-контракты и React TypeScript.</p>
      <p>До завершения общей проверки создание, редактирование и другие
      старые операции недоступны. Все прежние данные тестовые и при
      окончательном переходе будут удалены без переноса.</p>
      <p>Новый Rust Connect API проверяется отдельно. Старый REST не
      используется и автоматически не включается при ошибках.</p>
    </main>
  );
}
const root=document.getElementById("root");
if(!root)throw Error("CRM root missing");
createRoot(root).render(<ContractOnly/>);
