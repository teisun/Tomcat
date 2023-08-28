package com.tomcat.utils;
import cn.hutool.core.util.StrUtil;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.SignatureAlgorithm;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.core.userdetails.UserDetails;
import org.springframework.stereotype.Component;

import java.util.Date;
/**
 * jwt的工具类
 */
@Slf4j
@Component
public class JwtUtil {
    // 过期时间为3分钟
    @Value("${jwt.expiration}")
    private long EXPIRE_TIME = 365 * 24 * 60 * 60 * 1000;
    // 秘钥
    @Value("${jwt.SECRET_KEY}")
    private String TOKEN_SECRET_KEY;

    public static final String KEY_USER_ID = "userId";
    private static final String KEY_USER_NAME = "username";
    /**
     * 创建token
     *
     * @param userId
     * @param username
     * @return
     */
    public String generateToken(String userId, String username) {
        return Jwts.builder().setSubject(username)
                .claim(KEY_USER_ID, userId)
                .claim(KEY_USER_NAME, username)
                .setIssuedAt(new Date())
                .setExpiration(generateExpirationDate())
                .signWith(SignatureAlgorithm.HS256, TOKEN_SECRET_KEY)
                .compact();
    }

    /**
     * 生成token的过期时间
     */
    private Date generateExpirationDate() {
        return new Date(System.currentTimeMillis() + EXPIRE_TIME);
    }




    /**
     * 验证token
     *
     * @param token
     * @return
     */
    public Claims getClaims(String token) {
        Claims claims = null;
        try {
            claims = Jwts.parser()
                    .setSigningKey(TOKEN_SECRET_KEY)
                    .parseClaimsJws(token)
                    .getBody();
        } catch (Exception e) {
            log.info("JWT格式验证失败:{}",token);
        }
        return claims;
    }

    /**
     * 从token中获取key对应的值
     */
    public String getFieldFromToken(String key, String token) {
        String field;
        try {
            Claims claims = getClaims(token);
            field = (String) claims.get(key);
        } catch (Exception e) {
            field = null;
        }
        return field;
    }

    /**
     *
     *
     *  从token中获取登录用户名
     *
     */
    public String getUserNameFromToken(String token) {
        String username;
        try {
            Claims claims = getClaims(token);
            username = claims.getSubject();
        } catch (Exception e) {
            username = null;
        }
        return username;
    }

    /**
     * 校验token
     */
    public boolean validateToken(String token, UserDetails userDetails) {
        String username = getUserNameFromToken(token);
        String id = getFieldFromToken(KEY_USER_ID, token);
        return !StrUtil.isBlank(id) && username.equals(userDetails.getUsername()) && !isTokenExpired(token);
    }

    /**
     * 判断token是否已经失效
     */
    private boolean isTokenExpired(String token) {
        Date expiredDate = getClaims(token).getExpiration();
        return expiredDate.before(new Date());
    }



}