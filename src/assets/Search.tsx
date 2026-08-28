export const Search = ({
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
      width={width}
      height={height}
      viewBox="0 0 769 793"
    >
      <rect width="769" height="793" fill="none" />
      <path
        d="M340 40
           C176.2 40 43.5 172.7 43.5 336.5
           C43.5 500.3 176.2 633 340 633
           C413.1 633 480.1 606.5 530.5 562.7
           L703.9 736.1
           C714.7 746.9 732.3 746.9 743.1 736.1
           C753.9 725.3 753.9 707.7 743.1 696.9
           L569.1 522.9
           C612 471.7 637 406.1 637 336.5
           C637 172.7 504.3 40 340 40 Z
           M340 93
           C474.9 93 584 202.1 584 337
           C584 471.9 474.9 579 340 579
           C205.1 579 97 471.9 97 337
           C97 202.1 205.1 93 340 93 Z"
        fill={color}
        fill-rule="evenodd"
      />
    </svg>
  );
};
