package com.tomcat.service.impl;

import cn.hutool.core.convert.Convert;
import cn.hutool.core.util.StrUtil;
import com.tomcat.controller.requeset.AuthenticationReq;
import com.tomcat.controller.response.AuthenticationResp;
import com.tomcat.controller.response.UserResp;
import com.tomcat.domain.JwtUser;
import com.tomcat.domain.User;
import com.tomcat.domain.UserRepository;
import com.tomcat.service.UserService;
import com.tomcat.utils.JwtUtil;
import com.tomcat.utils.SecurityUtils;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.authentication.BadCredentialsException;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.core.userdetails.UserDetailsService;
import org.springframework.security.core.userdetails.UsernameNotFoundException;
import org.springframework.stereotype.Service;

import java.security.InvalidParameterException;
import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

@Service
public class UserServiceImpl implements UserService {

  @Autowired
  private UserRepository userRepository;

  @Autowired
  JwtUtil jwtUtil;

  @Autowired
  SecurityUtils securityUtils;

  @Autowired
  UserDetailsService userDetailsService;


  @Override
  public AuthenticationResp register(AuthenticationReq request) {

    // 密码编码
    String encodedPassword = securityUtils.encode(request.password);
    
    User user = new User();
    user.setUsername(request.username);
    user.setPassword(encodedPassword);
    user.setEmail(request.email);
    user.setPhoneNum(request.phoneNum);
    user.setDeviceId(request.deviceId);

    User userSaved = userRepository.save(user);
    String jwtToken = jwtUtil.generateToken(userSaved.getId(), userSaved.getUsername());

    return new AuthenticationResp(jwtToken);

  }

  @Override
  public AuthenticationResp login(AuthenticationReq request) {
    if(StrUtil.isBlank(request.username) || StrUtil.isBlank(request.phoneNum) || StrUtil.isBlank(request.email) || StrUtil.isBlank(request.password) || StrUtil.isBlank(request.deviceId)){
      throw new InvalidParameterException("user info must be not empty, you can set any string, like 'default' ");
    }
    User user = userRepository.findByUsernameOrPhoneNumOrEmail(request.username, request.phoneNum, request.email)
        .orElseThrow(() -> new RuntimeException("用户不存在"));


    // 校验密码
    if (!securityUtils.matches(request.password, user.getPassword())) {
      throw new BadCredentialsException("密码不正确");
    }

    // 设备ID不一致时更新User数据
    if(!request.deviceId.equals(user.getDeviceId())){
      user.setDeviceId(request.deviceId);
      userRepository.save(user);
    }

    JwtUser userDetails = new JwtUser(user);
    UsernamePasswordAuthenticationToken authentication = new UsernamePasswordAuthenticationToken(userDetails, null, userDetails.getAuthorities());
    SecurityContextHolder.getContext().setAuthentication(authentication);

    // 返回JWT令牌
    String jwtToken = jwtUtil.generateToken(user.getId(), user.getUsername());
    return new AuthenticationResp(jwtToken);
  }

  @Override
  public User findByUsername(String username) {
    return userRepository.findByUsername(username).orElseThrow(() -> new UsernameNotFoundException("User not found with username: " + username));
  }

  @Override
  public List<UserResp> findByDeviceId(String deviceId) {
    List<User> userList = userRepository.findByDeviceId(deviceId);
    return userList.stream()
            .map(user -> Convert.convert(UserResp.class, user))
            .collect(Collectors.toList());
  }

  @Override
  public AuthenticationResp registerOrLogin(AuthenticationReq request) {
      AuthenticationResp response;

//    Optional<User> userOptional = userRepository.findByUsername(request.username);
    Optional<User> userOptional = userRepository.findByUsernameOrPhoneNumOrEmail(request.username, request.phoneNum, request.email);

    if(!userOptional.isPresent()){
        if (request.username == null){
          request.username = request.deviceId;
        }
        response = register(request);
    }else {
        response = login(request);
    }
    return response;
  }

}