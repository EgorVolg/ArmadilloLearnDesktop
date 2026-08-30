export const ArrowDropDown = ({
  className,
  width = 34,
  height = 34,
}: {
  className?: string;
  width?: number;
  height?: number;
}) => {
  return (
    <svg
      className={className}
      xmlns="http://www.w3.org/2000/svg"
      height={height + "px"}
      viewBox="0 -960 960 960"
      width={width + "px"}
      fill="#F0000"
    >
      <path d="M480-360 280-560h400L480-360Z" />
    </svg>
  );
};

export const ArrowDropUp = ({
  className,
  width = 34,
  height = 34,
}: {
  className?: string;
  width?: number;
  height?: number;
}) => {
  return (
    <svg
      className={className}
      xmlns="http://www.w3.org/2000/svg"
      height={height + "px"}
      viewBox="0 -960 960 960"
      width={width + "px"}
      fill="#F0000"
    >
      <path d="m280-400 200-200 200 200H280Z" />
    </svg>
  );
};
