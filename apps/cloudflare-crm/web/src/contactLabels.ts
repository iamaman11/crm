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

