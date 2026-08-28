import Test from "../../../../assets/test.png";
import "./MainContent.css";

export interface Word {
  id: number;
  name: string;
  image: string;
  translations: string[];
  description: string;
}

const wordsData: Word[] = [
  {
    id: 0,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 1,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 2,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 3,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 4,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 5,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 6,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 7,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 8,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 9,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 10,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 11,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 12,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 13,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 14,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
];

interface MainContentProps {
  selected: boolean;
}

export const MainContent = ({ selected }: MainContentProps) => {
  return (
    <main className="main-app__main">
      <aside className="main-app__sidebar">
        <div className="main-app__sidebar-list-container">
          <div className="main-app__sidebar-title">Today</div>
          <ul className="main-app__sidebar-list">
            {wordsData.map((word) => (
              <li key={word.id} className="main-app__sidebar-item">
                {selected && (
                  <input
                    type="checkbox"
                    className="main-app__sidebar-item-checkbox"
                    checked={selected}
                  />
                )}
                <div className="main-app__sidebar-item-content">
                  <p className="main-app__sidebar-item-content-title">
                    {word.name + "-" + word.id}
                  </p>

                  <div className="main-app__sidebar-item-content-tags">
                    {word.translations.map((el, index) => (
                      <p
                        className="main-app__sidebar-item-content-tags-tag"
                        key={index}
                      >
                        {word.translations.length - 1 === index
                          ? el
                          : `${el}\u00A0·\u00A0`}
                      </p>
                    ))}
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </aside>

      <article className="main-app__article">
        <section className="main-app__article-section">
          <h1 className="main-app__article-section-title">
            {wordsData[0].name}
          </h1>
          <hr className="main-app__article-section-hr" />
          <p className="main-app__article-section-subtitle">
            {wordsData[0].translations[0]}
          </p>
        </section>
        <section className="main-app__article-section-content">
          <img
            src={Test}
            alt={wordsData[0].name}
            className="main-app__article-section-image"
          />
        </section>
        <section className="main-app__article-section-content">
          <div className="main-app__article-section-content-translations">
            A reply or answer to a request, message, or signal.
          </div>
        </section>
      </article>
    </main>
  );
};
