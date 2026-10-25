export function OnionMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="270 80 484 790"
      className={className}
      aria-hidden="true"
      fill="none"
    >
      <g transform="translate(0 36)">
        <path
          d="M512 190C540 250 590 290 650 350C730 430 760 520 740 610C720 710 640 790 512 806C384 790 304 710 284 610C264 520 294 430 374 350C434 290 484 250 512 190Z"
          fill="currentColor"
        />
        <g
          strokeWidth={14}
          strokeLinecap="round"
          style={{ stroke: "var(--bg)" }}
        >
          <path d="M512 232C420 332 360 450 372 590C382 690 440 770 512 806" />
          <path d="M512 232C604 332 664 450 652 590C642 690 584 770 512 806" />
          <path d="M512 262C466 350 436 460 446 580C454 670 486 750 512 800" />
          <path d="M512 262C558 350 588 460 578 580C570 670 538 750 512 800" />
          <path d="M512 306C500 400 498 520 512 790" />
        </g>
        <path
          d="M512 200C512 152 530 118 568 98"
          stroke="currentColor"
          strokeWidth={26}
          strokeLinecap="round"
        />
        <path
          d="M506 214C492 180 470 160 440 150"
          stroke="currentColor"
          strokeWidth={18}
          strokeLinecap="round"
        />
        <g
          stroke="currentColor"
          strokeWidth={12}
          strokeLinecap="round"
        >
          <path d="M512 806V850" />
          <path d="M488 802C480 826 468 842 452 856" />
          <path d="M536 802C544 826 556 842 572 856" />
        </g>
      </g>
    </svg>
  );
}
