import Logo from "../../../../assets/Logo.png";

import { ChooseThemeBtn } from "./ChooseThemeBtn";
import "./Header.css";

export interface HeaderNavItem {
  id: number;
  name: string;
  href: string;
}

const headerNavItems: HeaderNavItem[] = [
  {
    id: 1,
    name: "Words",
    href: "#",
  },
  {
    id: 2,
    name: "Translations & Dictionaries",
    href: "#",
  },
  {
    id: 3,
    name: "Settings",
    href: "#",
  },
];

interface HeaderProps {
  openedLink: number;
  onOpenLink: (id: number) => void;
}

export const Header = ({ openedLink, onOpenLink }: HeaderProps) => {
  return (
    <article className="main-app__header">
      <nav className="main-app__header-nav">
        <div className="main-app__header-logo">
          <img src={Logo} alt="logo" className="main-app__header-logo-image" />
          <h3 className="main-app__header-logo-text">Armadillo Learn</h3>
        </div>

        <ul className="main-app__header-nav-list">
          {headerNavItems.map((item) => (
            <li
              key={item.id}
              className={`${openedLink === item.id ? "main-app__header-nav-item-active" : "main-app__header-nav-item"}`}
              onClick={() => onOpenLink(item.id)}
            >
              <a href={item.href} className="main-app__header-nav-link">
                {item.name}
              </a>
            </li>
          ))}
        </ul>
        <ChooseThemeBtn />
      </nav>
    </article>
  );
};
