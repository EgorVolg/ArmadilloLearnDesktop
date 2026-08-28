import Logo from "../../../../assets/Logo.png";
import { PottedPlant } from "../../../../assets";
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
  onSwitchTheme: () => void;
}

export const Header = ({
  openedLink,
  onOpenLink,
  onSwitchTheme,
}: HeaderProps) => {
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

        <button
          className="main-app__header-nav-theme-switcher-button"
          onClick={onSwitchTheme}
        >
          <PottedPlant width={28} height={28} color="#d7b8a5" />
        </button>
      </nav>
    </article>
  );
};
