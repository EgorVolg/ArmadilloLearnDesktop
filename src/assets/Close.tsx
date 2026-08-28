export const Close = ({
  className,
  width = 34,
  height = 34,
  color = "#DFC0AD",
}: {
  className?: string;
  width?: number;
  height?: number;
  color?: string;
}) => {
  return (
    <svg
      className={className}
      xmlns="http://www.w3.org/2000/svg"
      height={height}
      viewBox="0 -960 960 960"
      width={width}
      fill={color}
    >
      <path d="m336-280-56-56 144-144-144-143 56-56 144 144 143-144 56 56-144 143 144 144-56 56-143-144-144 144Z" />
    </svg>
  );
};
